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
    client: std::sync::Arc<rspotify::ClientCredsSpotify>,
    data: rspotify::model::FullPlaylist,
    tracks: parking_lot::Mutex<Vec<Option<rspotify::model::PlayableItem>>>,
}

impl SpotifyPlaylist {
    pub async fn new(
        client: std::sync::Arc<rspotify::ClientCredsSpotify>,
        id: rspotify::model::PlaylistId<'_>,
    ) -> Result<Self, rspotify::ClientError> {
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
            "index {}, requesting {}..{}",
            index,
            subset_start,
            subset_start + subset_length,
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
