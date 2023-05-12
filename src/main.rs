mod json;
mod room_runner;
mod routes;
mod utils;

pub use json::*;
pub use room_runner::*;
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

/// Copy of all data that must be accessible from outside the room runner
pub struct RoomMeta {
    name: String,
    id: u32,
    player_ids: parking_lot::Mutex<Vec<PlayerId>>,
    password: Option<String>, // If None, room is public
    // explicit_songs: bool,
    num_rounds: u32,
    round_time_secs: u32,
    created_at: std::time::Instant,
    state: RoomState,
    theme: String,
}

struct Room {
    runner: AttachedTask,
    runner_tx: tokio::sync::mpsc::UnboundedSender<room_runner::RoomRunnerMessage>,
    meta: std::sync::Arc<RoomMeta>,
}

fn spawn_room(meta: RoomMeta) -> Room {
    let room_meta = std::sync::Arc::new(meta);
    let (runner_tx, runner_rx) = tokio::sync::mpsc::unbounded_channel();
    Room {
        runner: spawn_attached(room_runner::room_runner(room_meta.clone(), runner_rx)),
        runner_tx,
        meta: room_meta,
    }
}

pub struct State {
    rooms: parking_lot::Mutex<Vec<Room>>,
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
        rooms: parking_lot::Mutex::new(vec![spawn_room(RoomMeta {
            id: 420,
            name: "starter room lol".to_string(),
            player_ids: Vec::new(),
            password: None,
            num_rounds: 9,
            round_time_secs: 75,
            created_at: std::time::Instant::now(),
            state: RoomState::Lobby,
            theme: "Random Songs".into(),
        })]),
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
