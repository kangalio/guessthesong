use futures::{SinkExt as _, StreamExt as _};

use crate::redirect;

pub fn get_room(
    state: &crate::State,
    request: &tiny_http::Request,
    room_id_str: &str,
) -> Result<tiny_http::Response<std::io::Cursor<Vec<u8>>>, tiny_http::Response<std::io::Empty>> {
    let room_id_str = room_id_str.to_string();
    let room_id = room_id_str
        .parse::<u32>()
        .map_err(|_| tiny_http::Response::empty(400))?;
    let user_id = crate::extract_user_id_cookie(
        request
            .headers()
            .iter()
            .find(|h| h.field.equiv("Cookie"))
            .ok_or_else(|| tiny_http::Response::empty(400))?
            .value
            .as_str(),
    )
    .map_err(|_| tiny_http::Response::empty(400))?;

    let room_state;
    {
        let rooms = state.rooms.lock();
        let room = rooms
            .iter()
            .find(|r| r.id == room_id)
            .ok_or_else(|| redirect("http://127.0.0.1:5234/server-browser"))?;

        room.players
            .iter()
            .find(|p| p.id == user_id)
            .ok_or_else(|| redirect(&format!("http://127.0.0.1:5234/join/{}", room_id)))?;

        room_state = room.state.clone();
    };

    let html = match room_state {
        crate::RoomState::Lobby => std::fs::read_to_string("frontend/roomLOBBY.html")
            .map_err(|_| tiny_http::Response::empty(500))?
            .replace("ROOMID", &room_id_str)
            .replace("USERID", user_id),
        crate::RoomState::Play { .. } => std::fs::read_to_string("frontend/roomPLAY.html")
            .map_err(|_| tiny_http::Response::empty(500))?
            .replace("ROOMID", &room_id_str)
            .replace("PLAYERID", user_id),
    };
    Ok(tiny_http::Response::from_data(html).with_header(
        tiny_http::Header::from_bytes("Content-Type", "text/html").expect("can't fail"),
    ))
}

pub fn http_400(msg: &str) -> tungstenite::handshake::server::ErrorResponse {
    let mut response = tungstenite::handshake::server::ErrorResponse::new(Some(msg.to_string()));
    *response.status_mut() = http::StatusCode::BAD_REQUEST;
    response
}

#[derive(Clone)]
struct InitialRequest {
    room_id: u32,
    user_id: String,
    username: String,
}

fn parse_initial_request(
    state: &crate::State,
    req: &tungstenite::handshake::server::Request,
) -> Result<InitialRequest, tungstenite::handshake::server::ErrorResponse> {
    let room_id = req
        .uri()
        .path()
        .trim_start_matches("/")
        .parse::<u32>()
        .map_err(|_| http_400("bad path"))?;

    let cookie_header = req
        .headers()
        .get("Cookie")
        .ok_or_else(|| http_400("missing Cookie"))?
        .to_str()
        .map_err(|_| http_400("bad Cookie encoding"))?;
    let user_id = crate::extract_user_id_cookie(cookie_header)
        .map_err(http_400)?
        .to_string();

    let rooms = state.rooms.lock();
    let room = rooms
        .iter()
        .find(|r| r.id == room_id)
        .ok_or_else(|| http_400("that room doesn't exist"))?;
    let username = room
        .players
        .iter()
        .find(|p| p.id == user_id)
        .ok_or_else(|| http_400("you haven't joined this room"))?
        .name
        .clone();

    Ok(InitialRequest {
        room_id,
        user_id,
        username,
    })
}

async fn ws_send_to_all(state: &crate::State, room_id: u32, msg: &impl serde::Serialize) {
    let websockets = state
        .rooms
        .lock()
        .iter()
        .find(|r| r.id == room_id)
        .unwrap()
        .players
        .iter()
        .filter_map(|p| p.websocket.clone())
        .collect::<Vec<_>>();
    for websocket in websockets {
        ws_send(&mut *websocket.lock().await, msg).await;
    }
}

async fn ws_send(socket: &mut crate::WebsocketWrite, msg: impl serde::Serialize) {
    let msg = tungstenite::Message::Text(serde_json::to_string(&msg).expect("can't fail"));
    if let Err(e) = socket.send(msg).await {
        match e {
            tungstenite::Error::ConnectionClosed | tungstenite::Error::AlreadyClosed => {}
            e => log::error!("Failed to send WS message: {}", e),
        }
    }
}

async fn ws_recv<T: serde::de::DeserializeOwned>(socket: &mut crate::WebsocketRead) -> Option<T> {
    while let Some(Ok(msg)) = socket.next().await {
        match msg {
            tungstenite::Message::Text(msg) => match serde_json::from_str(&msg) {
                Ok(msg) => return msg,
                Err(e) => {
                    log::error!("malformed websocket message {:?}: {}", e, msg);
                    continue;
                }
            },
            tungstenite::Message::Close(_) => break,
            other => {
                log::info!("ignoring websocket message {:?}", other);
                continue;
            }
        }
    }

    None
}

fn player_state_msg(state: &crate::State, room_id: u32) -> serde_json::Value {
    let mut rooms = state.rooms.lock();
    let room = rooms
        .iter_mut()
        .find(|r| r.id == room_id)
        .expect("room was deleted inbetween websocket connection accept and first message");
    serde_json::json!( {
        "state": "join",
        // "message": username, // What does this do again?
        "payload": {
            "state": "player_data",
            "payload": room.players.iter().map(|player| serde_json::json!( {
                "uuid": player.id,
                "username": player.name,
                "points": 0,
                "streak": 0,
                "emoji": "😝",
                "prev_points": 0,
                "loaded": player.loaded,
                "guessed": false,
                "disconnected": false,
                "game_state": "Lobby"
            } )).collect::<Vec<_>>(),
            "owner": room.players.first().map_or("", |p| &p.id).to_string(),
        }
    } )
}

fn room(state: &crate::State, room_id: u32) -> impl std::ops::DerefMut<Target = crate::Room> + '_ {
    parking_lot::MutexGuard::map(state.rooms.lock(), |rooms| {
        rooms.iter_mut().find(|r| r.id == room_id).unwrap()
    })
}

fn player<'a>(
    state: &'a crate::State,
    room_id: u32,
    player_id: &str,
) -> impl std::ops::DerefMut<Target = crate::Player> + 'a {
    parking_lot::MutexGuard::map(state.rooms.lock(), |rooms| {
        rooms
            .iter_mut()
            .find(|r| r.id == room_id)
            .unwrap()
            .players
            .iter_mut()
            .find(|p| p.id == player_id)
            .unwrap()
    })
}

/// Returns title and path
fn select_random_song() -> crate::Song {
    let songs = std::fs::read_dir("/home/kangalioo/audio/maikel6311/nightcore/mp3/")
        .unwrap()
        .collect::<Vec<_>>();
    let random_index = crate::nanos_since_startup() % songs.len() as u128;
    let path = songs[random_index as usize].as_ref().unwrap().path();

    let title = path.file_name().unwrap().to_string_lossy();
    let title = title[..title.rfind('.').unwrap_or(title.len())].to_string();

    crate::Song { path, title }
}

async fn lobby_ws(
    state: &crate::State,
    websocket: tokio_tungstenite::WebSocketStream<tokio::net::TcpStream>,
    req_data: InitialRequest,
) {
    let InitialRequest {
        room_id,
        user_id,
        username,
    } = req_data;
    let (ws_write, mut ws_read) = websocket.split();

    room(state, room_id)
        .players
        .iter_mut()
        .find(|p| p.id == user_id)
        .unwrap()
        .websocket = Some(std::sync::Arc::new(tokio::sync::Mutex::new(ws_write)));

    std::thread::sleep(std::time::Duration::from_millis(200)); // HACK (to see traffic in firefox)
    ws_send_to_all(state, room_id, &player_state_msg(state, room_id)).await;

    #[derive(serde::Deserialize, Debug)]
    #[serde(tag = "type")]
    #[serde(rename_all = "kebab-case")]
    enum Message {
        IncomingMsg { msg: String },
        StartGame,
    }
    while let Some(msg) = ws_recv::<Message>(&mut ws_read).await {
        match msg {
            Message::IncomingMsg { msg } => {
                let msg = serde_json::json!( {
                    "type": "message",
                    "state": "chat",
                    "username": username,
                    "uuid": user_id,
                    "msg": msg,
                    "time_stamp": "Mar-25 09:42PM",
                } );
                ws_send_to_all(state, room_id, &msg).await;
            }
            Message::StartGame => {
                room(state, room_id).current_song = Some(select_random_song());

                let msg = serde_json::json!( { "state": "start_game" } );
                ws_send_to_all(state, room_id, &msg).await;
                {
                    let mut room = room(state, room_id);
                    room.state = crate::RoomState::Play;
                    for player in &mut room.players {
                        player.websocket = None;
                    }
                }
            }
        }
    }
}

fn generate_hints(title: &str, num_steps: usize) -> (String, Vec<String>) {
    fn blank_out_indices(s: &str, indices: &[usize]) -> String {
        s.chars()
            .enumerate()
            .map(|(i, c)| if indices.contains(&i) { '_' } else { c })
            .collect()
    }

    let mut indices_hidden = Vec::new();
    for (i, c) in title.chars().enumerate() {
        if c.is_alphanumeric() {
            indices_hidden.push(i);
        }
    }
    let all_blanked_out = blank_out_indices(title, &indices_hidden);

    let mut hints = Vec::new();

    let mut num_hidden = indices_hidden.len() as f32;
    let num_revealed_per_step = num_hidden / 2.0 / num_steps as f32;
    for _ in 0..num_steps {
        num_hidden -= num_revealed_per_step;
        while indices_hidden.len() as f32 > num_hidden {
            indices_hidden.remove(fastrand::usize(..indices_hidden.len()));
        }

        hints.push(blank_out_indices(title, &indices_hidden));
    }

    (all_blanked_out, hints)
}

async fn single_round(state: &crate::State, req_data: InitialRequest) {
    let InitialRequest {
        room_id,
        user_id: _,
        username: _,
    } = req_data;

    let song_title = room(state, room_id)
        .current_song
        .as_ref()
        .unwrap()
        .title
        .clone();
    let round_time = room(state, room_id).round_time_secs;

    let hints_at = (10..u32::min(round_time, 70)).step_by(10).rev();
    let (mut current_hint, hints) = generate_hints(&song_title, hints_at.len());
    let mut hints_at = hints_at
        .zip(hints)
        .collect::<std::collections::HashMap<_, _>>();

    tokio::time::sleep(std::time::Duration::from_millis(4000)).await;
    for timer in (0..=(round_time + 3)).rev() {
        tokio::time::sleep(std::time::Duration::from_millis(1000)).await;
        if room(state, room_id).players.iter().all(|p| p.guessed) {
            break;
        }

        if let Some(new_hint) = hints_at.remove(&timer) {
            current_hint = new_hint;
        }

        let msg = serde_json::json!( {
            "state": "timer",
            "message": timer,
            "hint": current_hint,
            "scores": room(state, room_id).players.iter().map(|p| serde_json::json!( {
                "uuid": p.id,
                "username": p.name,
                "points": p.points,
                "streak": 0,
                "emoji": "😝",
                "prev_points": 0,
                "loaded": false,
                "guessed": false,
                "disconnected": false,
                "game_state": "In game"
            } )).collect::<Vec<_>>(),
            "round_time": round_time
        } );
        ws_send_to_all(&state, room_id, &msg).await;
    }

    {
        let mut room = room(state, room_id);
        room.current_song = Some(select_random_song());
        for p in &mut room.players {
            p.loaded = false;
            p.guessed = false;
        }
    }

    let msg = serde_json::json!( {
        "state": "notify",
        "message": format!("The song was: {}", song_title),
        "type": "info",
    } );
    ws_send_to_all(&state, room_id, &msg).await;

    let msg = serde_json::json!( {
        "state": "new_turn",
    } );
    ws_send_to_all(&state, room_id, &msg).await;

    let msg = serde_json::json!( {
        "state": "scoreboard",
        "payload": room(state, room_id).players.iter().map(|p| serde_json::json!( {
            "uuid": p.id,
            "display_name": p.name,
            "points": p.points,
            "point_diff": 0,
            "prev_points": 0,
            "streak": 0
        } )).collect::<Vec<_>>(),
        "round": 1,
        "max_rounds": room(state, room_id).num_rounds,
        "turn": 0,
        "max_turn": 1
    } );
    ws_send_to_all(&state, room_id, &msg).await;
}

async fn ingame_ws(
    state: std::sync::Arc<crate::State>,
    websocket: tokio_tungstenite::WebSocketStream<tokio::net::TcpStream>,
    req_data: InitialRequest,
) {
    let InitialRequest {
        room_id,
        user_id,
        username,
    } = req_data.clone();
    let (mut ws_write, mut ws_read) = websocket.split();

    std::thread::sleep(std::time::Duration::from_millis(200)); // HACK (to see traffic in firefox)
    ws_send(&mut ws_write, player_state_msg(&state, room_id)).await;
    ws_send(&mut ws_write, serde_json::json!( { "state": "new_turn" } )).await;
    ws_send(&mut ws_write, serde_json::json!( { "state": "loading" } )).await;

    room(&state, room_id)
        .players
        .iter_mut()
        .find(|p| p.id == user_id)
        .unwrap()
        .websocket = Some(std::sync::Arc::new(tokio::sync::Mutex::new(ws_write)));

    #[derive(serde::Deserialize, Debug)]
    #[serde(tag = "type")]
    #[serde(rename_all = "kebab-case")]
    enum Message {
        IncomingMsg { msg: String },
        TypingStatus { typing: bool },
        AudioLoaded,
    }
    while let Some(msg) = ws_recv::<Message>(&mut ws_read).await {
        match msg {
            Message::IncomingMsg { msg } => {
                if msg.to_lowercase()
                    == room(&state, room_id)
                        .current_song
                        .as_ref()
                        .unwrap()
                        .title
                        .to_lowercase()
                {
                    room(&state, room_id)
                        .players
                        .iter_mut()
                        .find(|p| p.id == user_id)
                        .unwrap()
                        .guessed = true;
                } else {
                    let msg = serde_json::json!( {
                        "type": "message",
                        "state": "chat",
                        "username": username,
                        "uuid": user_id,
                        "msg": msg,
                        "time_stamp": "Mar-25 09:42PM",
                    } );
                    ws_send_to_all(&state, room_id, &msg).await;
                }
            }
            Message::TypingStatus { typing: _ } => {
                // STUB
            }
            Message::AudioLoaded => {
                player(&state, room_id, &user_id).loaded = true;
                ws_send_to_all(&state, room_id, &player_state_msg(&state, room_id)).await;

                let everyone_loaded = room(&state, room_id).players.iter().all(|p| p.loaded);
                if everyone_loaded {
                    let state = state.clone();
                    let req_data = req_data.clone();
                    tokio::spawn(async move {
                        single_round(&state, req_data).await;
                    });
                }
            }
        }
    }
}

pub async fn listen(state: std::sync::Arc<crate::State>) {
    let server = tokio::net::TcpListener::bind("0.0.0.0:9002")
        .await
        .expect("fatal: can't setup lobby websocket server");
    loop {
        let mut req_data = None;
        let ws = match server.accept().await {
            Ok((stream, _)) => match tokio_tungstenite::accept_hdr_async(
                stream,
                |req: &tungstenite::handshake::server::Request, res| match parse_initial_request(
                    &state, req,
                ) {
                    Ok(x) => {
                        req_data = Some(x);
                        Ok(res)
                    }
                    Err(error_response) => Err(error_response),
                },
            )
            .await
            {
                Ok(websocket) => websocket,
                Err(e) => {
                    log::error!("websocket connection failed: {}", e);
                    continue;
                }
            },
            Err(e) => {
                log::error!("websocket connection failed: {}", e);
                continue;
            }
        };
        let req_data = req_data.expect("websocket request callback wasn't called");

        let room_state = room(&state, req_data.room_id).state.clone();
        let state2 = state.clone();
        tokio::spawn(async move {
            match room_state {
                crate::RoomState::Lobby => lobby_ws(&state2, ws, req_data).await,
                crate::RoomState::Play { .. } => ingame_ws(state2, ws, req_data).await,
            }
        });
    }
}
