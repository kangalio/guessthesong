use rspotify::prelude::BaseClient as _;

#[derive(Clone)]
pub struct Song {
    pub title: String,
    pub audio: Vec<u8>,
}

#[derive(serde::Deserialize)]
struct YtdlpPlaylistEntry {
    url: String,
    title: String,
}

pub enum SongProvider {
    Spotify { spotify: rspotify::ClientCredsSpotify, tracks: Vec<rspotify::model::FullTrack> },
    Youtube { playlist: Vec<YtdlpPlaylistEntry> },
}

impl SongProvider {
    pub async fn new_spotify(playlist_id: &str) -> Self {
        let mut spotify = rspotify::ClientCredsSpotify::new(rspotify::Credentials {
            id: "0536121d4660414d9cc90962834cd390".into(),
            secret: Some("8a0f2d3327b749e39b9c50ed3deb218f".into()),
        });
        spotify.request_token().await.unwrap();

        let playlist = spotify
            .playlist(rspotify::model::PlaylistId::from_id(playlist_id).unwrap(), None, None)
            .await
            .unwrap();
        let tracks = playlist
            .tracks
            .items
            .into_iter()
            .filter_map(|item| match item.track {
                Some(rspotify::model::PlayableItem::Track(track)) => Some(track),
                _ => None,
            })
            .collect();
        Self::Spotify { spotify, tracks }
    }

    pub fn new_youtube() -> Self {
        let output = std::process::Command::new("yt-dlp")
            .arg("--dump-json")
            .arg("--flat-playlist")
            .arg("https://www.youtube.com/watch?v=2iRdKLodaXM&list=PL77D8FE68A35A932A")
            .output()
            .unwrap();
        let output = String::from_utf8_lossy(&output.stdout);

        let playlist = output.lines().map(|line| serde_json::from_str(line).unwrap()).collect();
        Self::Youtube { playlist }
    }

    pub async fn next(&self) -> Song {
        match self {
            SongProvider::Spotify { spotify, tracks } => {
                let track = &tracks[fastrand::usize(0..tracks.len())];

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
                yt_query += &track.name;

                let output = tokio::process::Command::new("yt-dlp")
                    .arg("-x")
                    .args(["-o", "-"])
                    .args(["--playlist-end", "1"])
                    // https://www.reddit.com/r/youtubedl/wiki/howdoidownloadpartsofavideo/
                    .args(["--download-sections", "*0-80"]) // 75s plus a few extra secs
                    .arg(yt_query)
                    .output()
                    .await
                    .unwrap();
                Song { title: track.name.clone(), audio: output.stdout }
            }
            SongProvider::Youtube { playlist } => {
                let song = &playlist[fastrand::usize(0..playlist.len())];

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
}
