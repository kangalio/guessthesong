use crate::spotify_playlist::*;

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
        regex::Regex::new(r"( \(.*\))?( - .*)?$").expect("impossible")
    });

    let Some(match_) = REGEX.find(title) else { return title.to_string() };
    title[..match_.start()].to_string()
}

enum Playlist {
    Spotify(SpotifyPlaylist),
    Youtube { tracks: Vec<YtdlpPlaylistEntry> },
}

async fn download_random_song(playlist: &Playlist) -> Song {
    match &playlist {
        Playlist::Spotify(playlist) => {
            let (artists, title) = match playlist.random_item().await {
                rspotify::model::PlayableItem::Track(track) => (
                    track.artists.iter().map(|x| &*x.name).collect::<Vec<_>>().join(", "),
                    track.name,
                ),
                rspotify::model::PlayableItem::Episode(episode) => {
                    (episode.show.publisher, episode.name)
                }
            };

            let output = tokio::process::Command::new("yt-dlp")
                .arg("-x")
                .args(["-o", "-"])
                .args(["--playlist-end", "1"])
                // https://www.reddit.com/r/youtubedl/wiki/howdoidownloadpartsofavideo/
                .args(["--download-sections", "*0-100"]) // 75s plus a few extra secs
                .arg(format!("https://music.youtube.com/search?q={artists} - {title}"))
                .output()
                .await
                .unwrap();

            Song { title: sanitize_spotify_title(&title), audio: output.stdout }
        }
        Playlist::Youtube { tracks } => {
            let song = tracks[fastrand::usize(0..tracks.len())].clone();

            let output = tokio::process::Command::new("yt-dlp")
                .arg("-x")
                .arg("-o")
                .arg("-")
                .arg(&song.url)
                .output()
                .await
                .unwrap();
            Song { title: song.title.clone(), audio: output.stdout }
        }
    }
}

pub struct SongProvider {
    playlist: std::sync::Arc<Playlist>,
    background_downloader: parking_lot::Mutex<tokio::task::JoinHandle<Song>>,
}

impl SongProvider {
    fn new(playlist: Playlist) -> Self {
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
            Playlist::Spotify(playlist) => playlist.name(),
            Playlist::Youtube { tracks: _ } => "[not implemented]",
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
        Ok(Self::new(Playlist::Spotify(
            SpotifyPlaylist::new(client, rspotify::model::PlaylistId::from_id(playlist_id)?)
                .await?,
        )))
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

        Ok(Self::new(Playlist::Youtube { tracks }))
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
