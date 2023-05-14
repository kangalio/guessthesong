pub mod hints;
pub mod room;
pub mod routes;
pub mod song_provider;
pub mod structs;
pub mod utils;

#[tokio::main]
async fn main() {
    env_logger::init();

    routes::run_axum().await;
}
