use crate::room::*;
use crate::song_provider::*;
use crate::structs::*;
use crate::utils::*;

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

    PlayerId(nanos_since_startup() as u64)
}

pub async fn get_server_browser(
    axum::extract::State(state): axum::extract::State<std::sync::Arc<State>>,
    ws: axum::extract::WebSocketUpgrade,
) -> impl axum::response::IntoResponse {
    ws.on_upgrade(move |ws| async move {
        let ws = WebSocket::new(ws);

        loop {
            if let Err(()) = ws.send(&SendEvent::FetchNew {
                msg: state
                    .rooms
                    .lock()
                    .iter()
                    .map(|(&id, room)| {
                        let room = room.lock();

                        ListedRoom {
                            code: id,
                            idle: (std::time::Instant::now() - room.created_at).as_secs(),
                            name: room.name.clone(),
                            players: room.players.len(),
                            status: if room.password.is_some() {
                                ListedRoomState::Private
                            } else {
                                ListedRoomState::Public
                            },
                            theme: room.theme.clone(),
                            game_mode: "Themes".into(), // is this ever something else?
                        }
                    })
                    .collect(),
            }) {
                break;
            }

            tokio::time::sleep(std::time::Duration::from_secs(5)).await;
        }
    })
}

pub async fn get_join(
    axum::extract::Path(room_id): axum::extract::Path<String>,
) -> Result<impl axum::response::IntoResponse, axum::response::ErrorResponse> {
    Ok(axum::response::Html(
        std::fs::read_to_string("frontend/join.html").unwrap().replace("ROOMID", &room_id),
    ))
}

#[derive(serde::Deserialize)]
pub struct PostJoinForm {
    username: String,
    room_code: u32,
}

pub async fn post_join(
    axum::extract::State(state): axum::extract::State<std::sync::Arc<State>>,
    axum::extract::TypedHeader(cookies): axum::extract::TypedHeader<axum::headers::Cookie>,
    axum::extract::Form(form): axum::extract::Form<PostJoinForm>,
) -> Result<impl axum::response::IntoResponse, axum::response::ErrorResponse> {
    let PostJoinForm { username, room_code } = form;
    let player_id = gen_id();
    let room = state
        .rooms
        .lock()
        .get(&room_code)
        .ok_or_else(|| axum::response::Redirect::to("/server-browser"))?
        .clone();

    let mut room = room.lock();
    room.players.push(Player {
        name: username.to_string(),
        id: player_id,
        loaded: false,
        guessed: None,
        points: 0,
        streak: 0,
        emoji: EMOJIS[cookies.get("emoji").unwrap().parse::<usize>().unwrap()].to_string(),
        ws: parking_lot::Mutex::new(None),
    });
    let player = room.players.last().expect("impossible, we just pushed");

    // Notify existing players about this newly joined user
    room.send_all(&room.player_state_msg());
    room.send_all(&SendEvent::Join {
        message: player.name.clone(),
        payload: Box::new(room.player_state_msg()),
    });

    Ok((
        axum::response::AppendHeaders([(
            axum::http::header::SET_COOKIE,
            format!("user={}; Path=/", player_id.0),
        )]),
        axum::response::Redirect::to(&format!("/room/{}", room_code)),
    ))
}

#[derive(serde::Deserialize)]
pub struct CreateRoomForm {
    username: String,
    room_name: String,
    playlist: String,
    password: String,
    rounds: u32,
    round_time: u32,
}

pub async fn post_create_room(
    axum::extract::State(state): axum::extract::State<std::sync::Arc<State>>,
    axum::extract::TypedHeader(cookies): axum::extract::TypedHeader<axum::headers::Cookie>,
    axum::extract::Form(form): axum::extract::Form<CreateRoomForm>,
) -> Result<impl axum::response::IntoResponse, axum::response::ErrorResponse> {
    let player_id = gen_id();

    let new_room = Room {
        name: form.room_name,
        password: if form.password.is_empty() { None } else { Some(form.password) },
        num_rounds: form.rounds,
        round_time_secs: form.round_time,
        created_at: std::time::Instant::now(),
        theme: "TODO".into(),
        song_provider: std::sync::Arc::new(SongProvider::from_any_url(&form.playlist).await),
        players: vec![Player {
            ws: parking_lot::Mutex::new(None),
            name: form.username,
            id: player_id,
            loaded: false,
            guessed: None,
            streak: 0,
            points: 0,
            emoji: EMOJIS[cookies.get("emoji").unwrap().parse::<usize>().unwrap()].to_string(),
        }],
        state: RoomState::Lobby,
        current_round: 0,
        round_task: None,
        current_song: None,
        round_start_time: None,
    };

    let mut rooms = state.rooms.lock();
    let new_room_id = rooms.keys().max().map_or(0, |largest_id| largest_id + 1);
    rooms.insert(new_room_id, std::sync::Arc::new(parking_lot::Mutex::new(new_room)));

    Ok((
        axum::response::AppendHeaders([(
            axum::http::header::SET_COOKIE,
            format!("user={}; Path=/", player_id.0),
        )]),
        axum::response::Redirect::to(&format!("/room/{}", new_room_id)),
    ))
}

#[derive(serde::Deserialize)]
pub struct RoomSettings {
    room_name: String,
    rounds: u32,
    round_time: u32,
}

fn get_or_post_room(
    state: std::sync::Arc<State>,
    cookies: axum::headers::Cookie,
    room_id: u32,
    apply_settings: Option<RoomSettings>,
) -> Result<impl axum::response::IntoResponse, axum::response::ErrorResponse> {
    let player_id = cookies
        .get("user")
        .ok_or_else(|| axum::response::Redirect::to(&format!("/join/{}", room_id)))?;
    let player_id = PlayerId(player_id.parse().unwrap());
    let room = state
        .rooms
        .lock()
        .get(&room_id)
        .ok_or_else(|| axum::response::Redirect::to("/server-browser"))?
        .clone();

    let mut room = room.lock();
    if !room.players.iter().any(|p| p.id == player_id) {
        return Err(axum::response::Redirect::to(&format!("/join/{}", room_id)).into());
    }

    if let Some(RoomSettings { room_name, rounds, round_time }) = apply_settings {
        room.name = room_name;
        room.num_rounds = rounds;
        room.round_time_secs = round_time;
    }

    let html = match room.state {
        RoomState::Lobby => std::fs::read_to_string("frontend/roomLOBBY.html")
            .map_err(|_| axum::http::StatusCode::INTERNAL_SERVER_ERROR)?
            .replace("ROOMID", &room_id.to_string())
            .replace("PLAYERID", &player_id.0.to_string()),
        RoomState::WaitingForLoaded | RoomState::WaitingForReconnect | RoomState::RoundStarted => {
            std::fs::read_to_string("frontend/roomPLAY.html")
                .map_err(|_| axum::http::StatusCode::INTERNAL_SERVER_ERROR)?
                .replace("ROOMID", &room_id.to_string())
                .replace("PLAYERID", &player_id.0.to_string())
        }
    };
    Ok(axum::response::Html(html))
}

pub async fn get_room(
    axum::extract::State(state): axum::extract::State<std::sync::Arc<State>>,
    axum::extract::TypedHeader(cookies): axum::extract::TypedHeader<axum::headers::Cookie>,
    axum::extract::Path(room_id): axum::extract::Path<u32>,
) -> Result<impl axum::response::IntoResponse, axum::response::ErrorResponse> {
    get_or_post_room(state, cookies, room_id, None)
}

pub async fn post_room(
    axum::extract::State(state): axum::extract::State<std::sync::Arc<State>>,
    axum::extract::TypedHeader(cookies): axum::extract::TypedHeader<axum::headers::Cookie>,
    axum::extract::Path(room_id): axum::extract::Path<u32>,
    axum::extract::Form(room_settings): axum::extract::Form<RoomSettings>,
) -> Result<impl axum::response::IntoResponse, axum::response::ErrorResponse> {
    get_or_post_room(state, cookies, room_id, Some(room_settings))
}

pub async fn get_room_ws(
    axum::extract::State(state): axum::extract::State<std::sync::Arc<State>>,
    axum::extract::Path(room_id): axum::extract::Path<u32>,
    axum::extract::TypedHeader(cookies): axum::extract::TypedHeader<axum::headers::Cookie>,
    ws: axum::extract::WebSocketUpgrade,
) -> impl axum::response::IntoResponse {
    let player_id = PlayerId(cookies.get("user").unwrap().parse().unwrap());
    let room = state.rooms.lock().get(&room_id).unwrap().clone();

    ws.on_upgrade(move |ws| async move {
        websocket_connect(room, player_id, std::sync::Arc::new(WebSocket::new(ws))).await;
    })
}

pub async fn get_song(
    axum::extract::State(state): axum::extract::State<std::sync::Arc<State>>,
    axum::extract::Path((_player_id, room_id, _random)): axum::extract::Path<(u32, u32, u32)>,
    axum::extract::TypedHeader(cookies): axum::extract::TypedHeader<axum::headers::Cookie>,
) -> impl axum::response::IntoResponse {
    let player_id = PlayerId(cookies.get("user").unwrap().parse().unwrap());
    let room = state.rooms.lock().get(&room_id).unwrap().clone();

    let room = room.lock();

    if !room.players.iter().any(|p| p.id == player_id) {
        return Err(axum::http::StatusCode::UNAUTHORIZED);
    }
    Ok(room.current_song.as_ref().unwrap().audio.clone())
}

pub async fn fallback(uri: axum::http::Uri) -> impl axum::response::IntoResponse {
    let mut path = uri.path().to_string();
    if path == "/" {
        path = "/index.html".to_string();
    }

    if let Ok(file) = tokio::fs::read_to_string(format!("frontend{}", &path)).await {
        return Ok(axum::response::Html(file));
    }
    if let Ok(file) = tokio::fs::read_to_string(format!("frontend{}.html", &path)).await {
        return Ok(axum::response::Html(file));
    }
    Err(axum::response::Redirect::to(&format!("https://guessthesong.io{}", uri.path())))
}

pub async fn run_axum() {
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
                round_task: None,
                song_provider: std::sync::Arc::new(
                    SongProvider::from_spotify_playlist("5wWUVh8qv6YygjbNZCckFl").await,
                ),
                theme: "Random Songs".into(),
                current_song: None,
                round_start_time: None,
                current_round: 0,
            })),
        )])),
    });

    let app = axum::Router::new()
        .route("/server-browser", axum::routing::get(get_server_browser))
        .route("/create-room.html", axum::routing::post(post_create_room))
        .route("/join/:room_id", axum::routing::get(get_join).post(post_join))
        .route("/room/:room_id", axum::routing::get(get_room).post(post_room))
        .route("/room/:room_id/ws", axum::routing::get(get_room_ws))
        .route("/song/:player_id/:room_id/:random", axum::routing::get(get_song))
        .fallback(fallback)
        .with_state(state);

    axum::Server::bind(&std::net::SocketAddr::from(([0, 0, 0, 0], 8787)))
        .serve(app.into_make_service())
        .await
        .unwrap();
}
