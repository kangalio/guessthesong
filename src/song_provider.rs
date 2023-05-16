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
    static REGEX: once_cell::sync::Lazy<regex::Regex> =
        once_cell::sync::Lazy::new(|| regex::Regex::new(r"( \(.*\))?( - .*)?$").unwrap());

    let Some(match_) = REGEX.find(title) else { return title.to_string() };
    title[..match_.start()].to_string()
}

enum Playlist {
    Spotify(Vec<rspotify::model::FullTrack>),
    Youtube(Vec<YtdlpPlaylistEntry>),
}

fn download_random_song_in_background(playlist: &Playlist) -> tokio::task::JoinHandle<Song> {
    match &playlist {
        Playlist::Spotify(tracks) => {
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
        Playlist::Youtube(playlist) => {
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
    playlist: Playlist,
    background_downloader: parking_lot::Mutex<tokio::task::JoinHandle<Song>>,
}

impl SongProvider {
    fn new(playlist: Playlist) -> Self {
        Self {
            background_downloader: parking_lot::Mutex::new(download_random_song_in_background(
                &playlist,
            )),
            playlist,
        }
    }

    pub async fn from_any_url(url: &str) -> Self {
        use once_cell::sync::Lazy;
        static SPOTIFY_URL_REGEX: Lazy<regex::Regex> =
            Lazy::new(|| regex::Regex::new("spotify.com/playlist/([^?/]+)").unwrap());
        if let Some(captures) = SPOTIFY_URL_REGEX.captures(url) {
            let playlist_id = captures.get(1).unwrap().as_str();
            return Self::from_spotify_playlist(playlist_id).await;
        }
        if url.contains("youtube.com") {
            return Self::from_youtube_playlist(url).await;
        }
        panic!("invalid URL: {}", url);
    }

    pub async fn from_spotify_playlist(playlist_id: &str) -> Self {
        let spotify = rspotify::ClientCredsSpotify::new(rspotify::Credentials {
            id: "0536121d4660414d9cc90962834cd390".into(),
            secret: Some("8a0f2d3327b749e39b9c50ed3deb218f".into()),
        });
        spotify.request_token().await.unwrap();

        let playlist = spotify
            .playlist(rspotify::model::PlaylistId::from_id(playlist_id).unwrap(), None, None)
            .await
            .unwrap();
        let playlist = playlist
            .tracks
            .items
            .into_iter()
            .filter_map(|item| match item.track {
                Some(rspotify::model::PlayableItem::Track(track)) => Some(track),
                _ => None,
            })
            .collect();

        Self::new(Playlist::Spotify(playlist))
    }

    pub async fn from_youtube_playlist(url: &str) -> Self {
        let output = tokio::process::Command::new("yt-dlp")
            .arg("--dump-json")
            .arg("--flat-playlist")
            .arg(url)
            .output()
            .await
            .unwrap();
        let output = String::from_utf8_lossy(&output.stdout);

        let playlist = output.lines().map(|line| serde_json::from_str(line).unwrap()).collect();

        Self::new(Playlist::Youtube(playlist))
    }

    pub async fn next(&self) -> Song {
        let background_downloader = std::mem::replace(
            &mut *self.background_downloader.lock(),
            download_random_song_in_background(&self.playlist),
        );
        background_downloader.await.unwrap()
    }
}
