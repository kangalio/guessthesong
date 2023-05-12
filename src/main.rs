mod json;
mod room;
mod routes;
mod utils;

pub use json::*;
pub use room::*;
pub use routes::*;
pub use utils::*;

const PLAYLIST: &str = "stuff/Das Gelbe vom Ei 2023.json";
const EMOJIS: &[&str] = &[
    "😀", "😃", "😄", "😁", "😆", "😅", "😂", "🤣", "😇", "🙂", "🙃", "😉", "😌", "😍", "😘", "😗",
    "😙", "😚", "😋", "😛", "😝", "😜", "🤪", "🤨", "🧐", "🤓", "😎", "🤩", "😏", "😒", "😞", "😔",
    "😟", "😕", "🙁", "☹️", "😣", "😖", "😫", "😩", "😢", "😭", "😤", "😠", "😡", "🤬", "🤯", "😳",
    "😱", "😨", "😰", "😥", "😓", "🤥", "😶", "😐", "😑", "😬", "🙄", "😯", "😦", "😧", "😮", "😲",
    "😴", "🤤", "😪", "😵", "🤐", "🤢", "🤮", "🤧", "😷", "🤒", "🤕", "🤑", "🤠", "😈", "👿", "👹",
    "👺", "🤡", "💩", "💀", "☠️", "👽", "👾", "🤖", "🎃", "😺", "😸", "😹", "😻", "😼", "😽", "🙀",
    "😿", "😾", "👶", "🧒", "👦", "👧", "🧑", "👩", "🧓", "👴", "👵", "🐶", "🐱", "🐭", "🐹", "🐰",
    "🦊", "🐻", "🐼", "🐨", "🐯", "🦁", "🐮", "🐷", "🐽", "🐸", "🐵", "🙈", "🙉", "🙊",
];

pub struct State {
    rooms: parking_lot::Mutex<
        std::collections::HashMap<u32, std::sync::Arc<parking_lot::Mutex<Room>>>,
    >,
}

fn gen_id() -> PlayerId {
    fn nanos_since_startup() -> u128 {
        thread_local! {
            static START_TIME: std::time::Instant = std::time::Instant::now();
        }
        START_TIME.with(|&start_time| std::time::Instant::now() - start_time).as_nanos()
    }

    PlayerId((nanos_since_startup() / 1000) as u32)
}

#[tokio::main]
async fn main() {
    env_logger::init();

    let state = std::sync::Arc::new(State {
        rooms: parking_lot::Mutex::new(From::from([(
            420,
            std::sync::Arc::new(parking_lot::Mutex::new(Room {
                name: "starter room lol".to_string(),
                players: Vec::new(),
                password: None,
                num_rounds: 9,
                round_time_secs: 75,
                created_at: std::time::Instant::now(),
                state: RoomState::Lobby,
                theme: "Random Songs".into(),
            })),
        )])),
    });

    let app = axum::Router::new()
        .route("/server-browser", axum::routing::get(routes::get_server_browser))
        .route("/join/:room_id", axum::routing::get(routes::get_join))
        .route("/join/:room_id", axum::routing::post(routes::post_join))
        .route("/room/:room_id", axum::routing::get(routes::get_room))
        .route("/room/:room_id/ws", axum::routing::get(routes::get_room_ws))
        .fallback(routes::fallback)
        .with_state(state);

    axum::Server::bind(&std::net::SocketAddr::from(([127, 0, 0, 1], 5234)))
        .serve(app.into_make_service())
        .await
        .unwrap();
}
