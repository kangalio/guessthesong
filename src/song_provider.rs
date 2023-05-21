use crate::spotify_playlist::*;
use crate::ytdlp_download::*;

#[derive(Clone)]
pub struct Song {
    pub title: String,
    pub audio: Vec<u8>,
}

#[derive(Clone, serde::Deserialize)]
struct YtdlpPlaylistEntry {
    url: String,
    title: String,
}

fn sanitize_spotify_title(title: &str) -> String {
    static REGEX: once_cell::sync::Lazy<regex::Regex> = once_cell::sync::Lazy::new(|| {
        regex::Regex::new(r"( \(.*\))?( \[.*\])?( - .*)?$").expect("impossible")
    });

    let Some(match_) = REGEX.find(title) else { return title.to_string() };
    title[..match_.start()].to_string()
}

#[cfg(test)]
#[test]
fn test_sanitize_spotify_title() {
    use sanitize_spotify_title as s;

    // First batch I used for the initial impl
    assert_eq!(s(r#"21 Reasons (feat. Ella Henderson)"#), "21 Reasons");
    assert_eq!(s(r#"Around the World (La La La La La) - Radio Version"#), "Around the World");
    assert_eq!(s(r#"Geordie - Radio"#), "Geordie");
    assert_eq!(s(r#"Funkytown - Single Version"#), "Funkytown");
    assert_eq!(s(r#"Jump - 2015 Remaster"#), "Jump");
    assert_eq!(s(r#"You Spin Me Round (Like a Record)"#), "You Spin Me Round");
    assert_eq!(s(r#"Start Me Up - Remastered 2009"#), "Start Me Up");
    assert_eq!(s(r#"Jump (Original Mix)"#), "Jump");
    assert_eq!(s(r#"Lambada - Original Radio Edit"#), "Lambada");
    assert_eq!(s(r#"Rock with You - Single Version"#), "Rock with You");
    assert_eq!(s(r#"Wonderwall - Remastered"#), "Wonderwall");
    assert_eq!(s(r#"Blue (Da Ba Dee)"#), "Blue");
    assert_eq!(s(r#"Blue (Da Ba Dee) - Gabry Ponte Ice Pop Radio"#), "Blue");
    assert_eq!(s(r#"My Heart Will Go On - Love Theme from "Titanic""#), "My Heart Will Go On");
    assert_eq!(s(r#"What Is Love - 7" Mix"#), "What Is Love");
    assert_eq!(s(r#"Everybody (Backstreet's Back) - Radio Edit"#), "Everybody");
    assert_eq!(s(r#"Mr. Vain - Original Radio Edit"#), "Mr. Vain");
    assert_eq!(s(r#"Ecuador - Original Radio Edit"#), "Ecuador");
    assert_eq!(s(r#"I'm Outta Love - Radio Edit"#), "I'm Outta Love");
    assert_eq!(s(r#"Saturday Night - Radio Mix"#), "Saturday Night");
    assert_eq!(s(r#"9Pm (Till I Come)"#), "9Pm");
    assert_eq!(s(r#"Men In Black - From "Men In Black" Soundtrack"#), "Men In Black");

    assert_eq!(
        s(r#"Du Hast Den Farbfilm Vergessen (Radio Edit) [feat. Stephanie Kurpisch]"#),
        "Du Hast Den Farbfilm Vergessen"
    );
}

enum PlaylistSource {
    Spotify { playlist: SpotifyPlaylist, indices_not_played_yet: parking_lot::Mutex<Vec<usize>> },
    Youtube { tracks: Vec<YtdlpPlaylistEntry> },
}

async fn download_random_song(playlist: &PlaylistSource) -> Song {
    match playlist {
        PlaylistSource::Spotify { playlist, indices_not_played_yet } => {
            // Select random track
            let track_index = {
                let mut indices_not_played_yet = indices_not_played_yet.lock();
                if indices_not_played_yet.is_empty() {
                    log::info!("dang either the playlist is tiny or the lobby runs very long");
                    *indices_not_played_yet = (0..playlist.len()).collect();
                }
                let x = fastrand::usize(..indices_not_played_yet.len());
                indices_not_played_yet.remove(x)
            };
            let track = playlist.track(track_index).await.expect("index cant be out of bounds");

            // Build youtube search query
            let (artists, title) = match &track {
                rspotify::model::PlayableItem::Track(track) => {
                    (track.artists.iter().map(|x| &*x.name).collect::<Vec<_>>(), &*track.name)
                }
                rspotify::model::PlayableItem::Episode(episode) => {
                    (vec![&*episode.show.publisher], &*episode.name)
                }
            };

            Song {
                title: sanitize_spotify_title(&title),
                audio: download_best_effort(&artists, &title).await,
            }
        }
        PlaylistSource::Youtube { tracks } => {
            let song = tracks[fastrand::usize(0..tracks.len())].clone();

            Song { title: song.title.clone(), audio: download_url(&song.url).await }
        }
    }
}

pub struct SongProvider {
    playlist: std::sync::Arc<PlaylistSource>,
    background_downloader: parking_lot::Mutex<tokio::task::JoinHandle<Song>>,
}

impl SongProvider {
    fn new(playlist: PlaylistSource) -> Self {
        let playlist = std::sync::Arc::new(playlist);
        let playlist2 = playlist.clone();
        Self {
            background_downloader: parking_lot::Mutex::new(tokio::spawn(async move {
                download_random_song(&*playlist2).await
            })),
            playlist,
        }
    }

    pub fn playlist_name(&self) -> &str {
        match &*self.playlist {
            PlaylistSource::Spotify { playlist, .. } => playlist.name(),
            PlaylistSource::Youtube { tracks: _ } => "[not implemented]",
        }
    }

    pub async fn from_any_url(
        client: std::sync::Arc<rspotify::ClientCredsSpotify>,
        url: &str,
    ) -> Result<Self, Box<dyn std::error::Error + Send + Sync>> {
        use once_cell::sync::Lazy;
        static SPOTIFY_URL_REGEX: Lazy<regex::Regex> =
            Lazy::new(|| regex::Regex::new("spotify.com/playlist/([^?/]+)").expect("impossible"));
        if let Some(captures) = SPOTIFY_URL_REGEX.captures(url) {
            let playlist_id = captures.get(1).expect("impossible").as_str();
            return Ok(Self::from_spotify_playlist(client, playlist_id).await?);
        }
        if url.contains("youtube.com") {
            return Ok(Self::from_youtube_playlist(url).await?);
        }
        Err(format!("invalid URL: {}", url).into())
    }

    pub async fn from_spotify_playlist(
        client: std::sync::Arc<rspotify::ClientCredsSpotify>,
        playlist_id: &str,
    ) -> Result<Self, Box<dyn std::error::Error + Send + Sync>> {
        let playlist =
            SpotifyPlaylist::new(client, rspotify::model::PlaylistId::from_id(playlist_id)?)
                .await?;
        Ok(Self::new(PlaylistSource::Spotify {
            indices_not_played_yet: parking_lot::Mutex::new((0..playlist.len()).collect()),
            playlist,
        }))
    }

    pub async fn from_youtube_playlist(
        url: &str,
    ) -> Result<Self, Box<dyn std::error::Error + Send + Sync>> {
        let output = tokio::process::Command::new("yt-dlp")
            .arg("--dump-json")
            .arg("--flat-playlist")
            .arg(url)
            .output()
            .await?;
        let output = String::from_utf8_lossy(&output.stdout);

        let tracks =
            output.lines().map(|line| serde_json::from_str(line)).collect::<Result<_, _>>()?;

        Ok(Self::new(PlaylistSource::Youtube { tracks }))
    }

    pub async fn next(&self) -> Song {
        let playlist = self.playlist.clone();
        let prev_background_downloader = std::mem::replace(
            &mut *self.background_downloader.lock(),
            tokio::spawn(async move { download_random_song(&playlist).await }),
        );
        prev_background_downloader.await.expect("downloader panicked or was cancelled?")
    }
}
