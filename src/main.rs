pub mod hints;
pub mod room_flow;
pub mod room_structs;
pub mod routes;
pub mod song_provider;
pub mod utils;
pub mod ws_structs;

#[tokio::main]
async fn main() {
    env_logger::init();

    routes::run_axum().await;
}
