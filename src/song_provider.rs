#[derive(Clone)]
pub struct Song {
    pub title: String,
    pub audio: Vec<u8>,
}

/* use rspotify::prelude::BaseClient;

pub struct SongProvider {
    // playlist_id: rspotify::model::PlaylistId,
    spotify: rspotify::ClientCredsSpotify,
}

impl SongProvider {
    pub fn new() -> Self {
        Self {
            // playlist_id,
            spotify: rspotify::ClientCredsSpotify::new(rspotify::Credentials {
                id: "0536121d4660414d9cc90962834cd390".into(),
                secret: Some("8a0f2d3327b749e39b9c50ed3deb218f".into()),
            }),
        }
    }

    pub async fn next(&self) -> Song {
        let playlist = self
            .spotify
            .playlist(
                rspotify::model::PlaylistId::from_id("3cEYpjA9oz9GiPac4AsH4n").unwrap(),
                None,
                None,
            )
            .await
            .unwrap();


        let track = &playlist.tracks.items[fastrand::usize(0..playlist.tracks.items.len())];
        let rspotify::model::PlayableItem::Track(track) = track.unwrap() else { panic!() };
        track.
    }
} */

#[derive(serde::Deserialize)]
struct YtdlpPlaylistEntry {
    url: String,
    title: String,
}

pub struct SongProvider {
    playlist: Vec<YtdlpPlaylistEntry>,
}

impl SongProvider {
    pub fn new() -> Self {
        let output = std::process::Command::new("yt-dlp")
            .arg("--dump-json")
            .arg("--flat-playlist")
            .arg("https://www.youtube.com/watch?v=2iRdKLodaXM&list=PL77D8FE68A35A932A")
            .output()
            .unwrap();
        let output = String::from_utf8_lossy(&output.stdout);

        Self { playlist: output.lines().map(|line| serde_json::from_str(line).unwrap()).collect() }
    }

    pub async fn next(&self) -> Song {
        let song = &self.playlist[fastrand::usize(0..self.playlist.len())];

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
