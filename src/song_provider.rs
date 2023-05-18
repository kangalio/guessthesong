use rspotify::prelude::BaseClient as _;

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

enum Tracks {
    Spotify(Vec<rspotify::model::FullTrack>),
    Youtube(Vec<YtdlpPlaylistEntry>),
}

fn download_random_song_in_background(playlist: &Tracks) -> tokio::task::JoinHandle<Song> {
    match &playlist {
        Tracks::Spotify(tracks) => {
            let track = &tracks[fastrand::usize(0..tracks.len())];
            let title = track.name.clone();

            // "...?q=$($artist),* - $title"
            let mut yt_query = "https://music.youtube.com/search?q=".to_string();
            let mut artists = track.artists.iter();
            if let Some(artist) = artists.next() {
                yt_query += &artist.name;
            }
            for artist in artists {
                yt_query += ", ";
                yt_query += &artist.name;
            }
            yt_query += " - ";
            yt_query += &title;

            tokio::spawn(async move {
                let output = tokio::process::Command::new("yt-dlp")
                    .arg("-x")
                    .args(["-o", "-"])
                    .args(["--playlist-end", "1"])
                    // https://www.reddit.com/r/youtubedl/wiki/howdoidownloadpartsofavideo/
                    .args(["--download-sections", "*0-100"]) // 75s plus a few extra secs
                    .arg(yt_query)
                    .output()
                    .await
                    .unwrap();

                Song { title: sanitize_spotify_title(&title), audio: output.stdout }
            })
        }
        Tracks::Youtube(playlist) => {
            let song = playlist[fastrand::usize(0..playlist.len())].clone();

            tokio::spawn(async move {
                let output = tokio::process::Command::new("yt-dlp")
                    .arg("-x")
                    .arg("-o")
                    .arg("-")
                    .arg(&song.url)
                    .output()
                    .await
                    .unwrap();
                Song { title: song.title.clone(), audio: output.stdout }
            })
        }
    }
}

pub struct SongProvider {
    tracks: Tracks,
    playlist_name: String,
    background_downloader: parking_lot::Mutex<tokio::task::JoinHandle<Song>>,
}

impl SongProvider {
    fn new(tracks: Tracks, playlist_name: String) -> Self {
        Self {
            background_downloader: parking_lot::Mutex::new(download_random_song_in_background(
                &tracks,
            )),
            tracks,
            playlist_name,
        }
    }

    pub fn playlist_name(&self) -> &str {
        &self.playlist_name
    }

    pub async fn from_any_url(url: &str) -> Result<Self, Box<dyn std::error::Error + Send + Sync>> {
        use once_cell::sync::Lazy;
        static SPOTIFY_URL_REGEX: Lazy<regex::Regex> =
            Lazy::new(|| regex::Regex::new("spotify.com/playlist/([^?/]+)").expect("impossible"));
        if let Some(captures) = SPOTIFY_URL_REGEX.captures(url) {
            let playlist_id = captures.get(1).expect("impossible").as_str();
            return Ok(Self::from_spotify_playlist(playlist_id).await?);
        }
        if url.contains("youtube.com") {
            return Ok(Self::from_youtube_playlist(url).await?);
        }
        Err(format!("invalid URL: {}", url).into())
    }

    pub async fn from_spotify_playlist(
        playlist_id: &str,
    ) -> Result<Self, Box<dyn std::error::Error + Send + Sync>> {
        let spotify = rspotify::ClientCredsSpotify::new(rspotify::Credentials {
            id: "0536121d4660414d9cc90962834cd390".into(),
            secret: Some("8a0f2d3327b749e39b9c50ed3deb218f".into()),
        });
        spotify.request_token().await?;

        let playlist = spotify
            .playlist(rspotify::model::PlaylistId::from_id(playlist_id)?, None, None)
            .await?;
        let tracks = playlist
            .tracks
            .items
            .into_iter()
            .filter_map(|item| match item.track {
                Some(rspotify::model::PlayableItem::Track(track)) => Some(track),
                _ => None,
            })
            .collect();

        Ok(Self::new(Tracks::Spotify(tracks), playlist.name))
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

        Ok(Self::new(Tracks::Youtube(tracks), "<YouTube playlist>".into()))
    }

    pub async fn next(&self) -> Song {
        let background_downloader = std::mem::replace(
            &mut *self.background_downloader.lock(),
            download_random_song_in_background(&self.tracks),
        );
        background_downloader.await.expect("downloader panicked or was cancelled?")
    }
}
