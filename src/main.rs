mod hints;
mod room;
mod routes;
mod song_provider;
mod structs;
mod utils;

#[tokio::main]
async fn main() {
    env_logger::init();

    routes::run_axum().await;
}
