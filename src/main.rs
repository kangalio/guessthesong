mod hints;
mod room_flow;
mod room_structs;
mod routes;
mod song_provider;
mod spotify_playlist;
mod utils;
mod ws_structs;
mod ytdlp_download;

#[tokio::main]
async fn main() {
    dotenv::dotenv().ok();
    env_logger::init();

    // Setup Spotify client
    let creds = rspotify::Credentials {
        id: std::env::var("SPOTIFY_ID").expect("missing SPOTIFY_ID env variable"),
        secret: Some(std::env::var("SPOTIFY_SECRET").expect("missing SPOTIFY_SECRET env variable")),
    };
    let config = rspotify::Config {
        token_refreshing: true, // not enabled by default for ??? reasons
        ..Default::default()
    };
    let spotify_client = rspotify::ClientCredsSpotify::with_config(creds, config);
    spotify_client.request_token().await.expect("invalid Spotify credentials");

    // Run HTTP server
    routes::run_axum(spotify_client).await;
}
