mod hints;
mod room_flow;
mod room_structs;
mod routes;
mod song_provider;
mod utils;
mod ws_structs;
mod spotify_playlist;

#[tokio::main]
async fn main() {
    env_logger::init();

    routes::run_axum().await;
}
