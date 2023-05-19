use rspotify::prelude::BaseClient as _;

fn insert_page(
    cache: &mut [Option<rspotify::model::PlayableItem>],
    page: rspotify::model::Page<rspotify::model::PlaylistItem>,
) {
    for (slot, item) in cache[page.offset as usize..].iter_mut().zip(page.items) {
        *slot = Some(item.track.expect("https://github.com/ramsayleung/rspotify/issues/411"));
    }
}

pub struct SpotifyPlaylist {
    client: rspotify::ClientCredsSpotify,
    data: rspotify::model::FullPlaylist,
    tracks: parking_lot::Mutex<Vec<Option<rspotify::model::PlayableItem>>>,
}

impl SpotifyPlaylist {
    pub async fn new(id: rspotify::model::PlaylistId<'_>) -> Result<Self, rspotify::ClientError> {
        let client = rspotify::ClientCredsSpotify::new(rspotify::Credentials {
            id: "0536121d4660414d9cc90962834cd390".into(),
            secret: Some("8a0f2d3327b749e39b9c50ed3deb218f".into()),
        });
        client.request_token().await?;

        let mut data = client.playlist(id, None, None).await?;

        let mut tracks = vec![None; data.tracks.total as usize];
        insert_page(&mut tracks, std::mem::take(&mut data.tracks));

        Ok(Self { client, tracks: parking_lot::Mutex::new(tracks), data })
    }

    pub fn name(&self) -> &str {
        &self.data.name
    }

    pub async fn random_item(&self) -> rspotify::model::PlayableItem {
        let index;
        {
            let tracks = self.tracks.lock();
            index = fastrand::usize(..tracks.len());
            if let Some(track) = &tracks[index] {
                log::info!("index {} is already cached, nice", index);
                return track.clone();
            }
        }

        let subset_start = index / 50 * 50;
        let subset_length = 50; // Spotify API's maximum
        log::info!(
            "needing track {} requesting tracks {}..{} from {}",
            index,
            subset_start,
            subset_start + subset_length,
            self.data.id
        );
        let page = self
            .client
            .playlist_items_manual(
                self.data.id.as_ref(),
                None,
                None,
                Some(subset_length as u32),
                Some(subset_start as u32),
            )
            .await
            .unwrap();

        {
            let mut tracks = self.tracks.lock();
            insert_page(&mut tracks, page);
            tracks[index].clone().expect("we just inserted")
        }
    }
}
