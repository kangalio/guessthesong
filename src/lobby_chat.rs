use futures::{SinkExt as _, StreamExt as _};

use crate::redirect;

pub fn get_room(
    state: &crate::State,
    cookie_header: &str,
    room_id_str: &str,
) -> Result<tiny_http::Response<std::io::Cursor<Vec<u8>>>, tiny_http::Response<std::io::Empty>> {
    let room_id_str = room_id_str.to_string();
    let room_id = room_id_str
        .parse::<u32>()
        .map_err(|_| tiny_http::Response::empty(400))?;
    let user_id = crate::extract_cookie(cookie_header, "user")
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
            .replace("USERID", &user_id),
        crate::RoomState::Play { .. } => std::fs::read_to_string("frontend/roomPLAY.html")
            .map_err(|_| tiny_http::Response::empty(500))?
            .replace("ROOMID", &room_id_str)
            .replace("PLAYERID", &user_id),
    };
    Ok(tiny_http::Response::from_data(html).with_header(
        tiny_http::Header::from_bytes("Content-Type", "text/html").expect("can't fail"),
    ))
}

pub fn http_400(msg: impl Into<String>) -> tungstenite::handshake::server::ErrorResponse {
    let mut response = tungstenite::handshake::server::ErrorResponse::new(Some(msg.into()));
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
    let user_id = crate::extract_cookie(cookie_header, "user")
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

fn player_state_msg(state: &crate::State, room_id: u32, joined: Option<&str>) -> serde_json::Value {
    let mut rooms = state.rooms.lock();
    let room = rooms
        .iter_mut()
        .find(|r| r.id == room_id)
        .expect("room was deleted inbetween websocket connection accept and first message");
    serde_json::json!( {
        "state": if joined.is_some() { "join" } else { "player_data" },
        "message": joined,
        "payload": {
            "state": "player_data",
            "payload": room.players.iter().map(|p| serde_json::json!( {
                "uuid": p.id,
                "username": p.name,
                "points": p.points + p.guessed.unwrap_or(0),
                "streak": p.streak,
                "emoji": p.emoji,
                "prev_points": p.points,
                "loaded": p.loaded,
                "guessed": p.guessed.is_some(),
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
    let songs = serde_json::from_str::<Vec<crate::Song>>(
        &std::fs::read_to_string("Das Gelbe vom Ei 2019.json").unwrap(),
    )
    .unwrap();
    let random_index = crate::nanos_since_startup() % songs.len() as u128;
    let mut song = songs[random_index as usize].clone();

    println!("Starting song download...");
    let audio = std::sync::Arc::new(
        std::process::Command::new("yt-dlp")
            .args([
                "-x",
                "-o",
                "-",
                "--playlist-end",
                "1",
                &format!(
                    "https://music.youtube.com/search?q={} - {}",
                    song.artist, song.title
                ),
            ])
            .output()
            .unwrap()
            .stdout,
    );
    println!("Finished song download!");
    song.audio = Some(audio.clone());

    song
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
    ws_send_to_all(
        state,
        room_id,
        &player_state_msg(state, room_id, Some(&username)),
    )
    .await;

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

    let song = room(state, room_id).current_song.clone().unwrap();
    let song_title = &song.title;
    let round_time = room(state, room_id).round_time_secs;

    let hints_at = (10..u32::min(round_time, 70)).step_by(10).rev();
    let (mut current_hint, hints) = generate_hints(&song_title, hints_at.len());
    let mut hints_at = hints_at
        .zip(hints)
        .collect::<std::collections::HashMap<_, _>>();

    tokio::time::sleep(std::time::Duration::from_millis(4000)).await;
    for timer in (0..=(round_time + 3)).rev() {
        tokio::time::sleep(std::time::Duration::from_millis(1000)).await;
        if room(state, room_id)
            .players
            .iter()
            .all(|p| p.guessed.is_some())
        {
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
                "points": p.points + p.guessed.unwrap_or(0),
                "streak": p.streak,
                "emoji": p.emoji,
                "prev_points": p.points,
                "loaded": p.loaded,
                "guessed": p.guessed.is_some(),
                "disconnected": false,
                "game_state": "In game"
            } )).collect::<Vec<_>>(),
            "round_time": round_time
        } );
        ws_send_to_all(&state, room_id, &msg).await;

        if timer == round_time {
            room(&state, room_id).round_start_time = Some(std::time::Instant::now());
        }
    }

    {
        let mut room = room(state, room_id);
        for p in &mut room.players {
            if let Some(new_points) = p.guessed {
                p.points += new_points;
                p.streak += 1;
            } else {
                p.streak = 0;
            }
        }
    }

    let msg = serde_json::json!( {
        "state": "notify",
        "message": format!("The song was: {}", song_title),
        "type": "info",
    } );
    ws_send_to_all(&state, room_id, &msg).await;

    let msg = serde_json::json!( {
        "state": "scoreboard",
        "payload": room(state, room_id).players.iter().map(|p| serde_json::json!( {
            "uuid": p.id,
            "display_name": p.name,
            "points": p.points,
            "point_diff": p.guessed.unwrap_or(0),
            "prev_points": p.points - p.guessed.unwrap_or(0),
            "streak": p.streak,
        } )).collect::<Vec<_>>(),
        "round": room(state, room_id).current_round + 1,
        "max_rounds": room(state, room_id).num_rounds,
    } );
    ws_send_to_all(&state, room_id, &msg).await;

    {
        let mut room = room(state, room_id);
        room.current_song = Some(select_random_song());
        for p in &mut room.players {
            p.guessed = None;
            p.loaded = false;
        }
        room.current_round += 1;
    }

    let msg = serde_json::json!( {
        "state": "new_turn",
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
    ws_send(&mut ws_write, player_state_msg(&state, room_id, None)).await;
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
                    let guess_time = (std::time::Instant::now()
                        - room(&state, room_id).round_start_time.unwrap())
                    .as_secs_f32();
                    let how_many_others_have_already_guessed = room(&state, room_id)
                        .players
                        .iter()
                        .filter(|p| p.guessed.is_some())
                        .count();
                    let hints_left =
                        ((room(&state, room_id).round_time_secs as f32 - guess_time) / 10.0) as u32;

                    // This is the original GuessTheSong algorithm as posted by "Frank (11studios)"
                    // in the GuessTheSong.io Discord server
                    // https://discord.com/channels/741670496822886470/741670497304969232/1092483679261053078
                    let mut points = 100;
                    match guess_time {
                        x if x < 10.0 => points += 125,
                        x if x < 20.0 => points += 100,
                        x if x < 25.0 => points += 75,
                        x if x < 45.0 => points += 62,
                        x if x < 70.0 => points += 50,
                        _ => points += 25,
                    }
                    match how_many_others_have_already_guessed {
                        0 => points += 200,
                        1 => points += 150,
                        2 => points += 100,
                        _ => {}
                    }
                    points += u32::min(hints_left * 25, 100);

                    room(&state, room_id)
                        .players
                        .iter_mut()
                        .find(|p| p.id == user_id)
                        .unwrap()
                        .guessed = Some(points);
                } else {
                    let msg = serde_json::json!( {
                        "type": "message",
                        "state": "chat",
                        "username": username,
                        "uuid": user_id,
                        "msg": msg,
                    } );
                    ws_send_to_all(&state, room_id, &msg).await;
                }
            }
            Message::TypingStatus { typing } => {
                let msg = serde_json::json!( {
                    "state": "playerTyping",
                    "uuid": user_id,
                    "typing": typing,
                } );
                ws_send_to_all(&state, room_id, &msg).await;
            }
            Message::AudioLoaded => {
                player(&state, room_id, &user_id).loaded = true;
                ws_send_to_all(&state, room_id, &player_state_msg(&state, room_id, None)).await;

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
