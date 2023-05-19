mod hints;
mod room_flow;
mod room_structs;
mod routes;
mod song_provider;
mod spotify_playlist;
mod utils;
mod ws_structs;

#[tokio::main]
async fn main() {
    dotenv::dotenv().ok();
    env_logger::init();

    let spotify_client = rspotify::ClientCredsSpotify::new(rspotify::Credentials {
        id: std::env::var("SPOTIFY_ID").expect("missing SPOTIFY_ID env variable"),
        secret: Some(std::env::var("SPOTIFY_SECRET").expect("missing SPOTIFY_SECRET env variable")),
    });
    spotify_client.request_token().await.expect("invalid Spotify credentials");

    routes::run_axum(spotify_client).await;
}
