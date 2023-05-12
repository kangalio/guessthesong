use crate::*;

pub async fn get_server_browser(
    axum::extract::State(state): axum::extract::State<std::sync::Arc<crate::State>>,
    ws: axum::extract::WebSocketUpgrade,
) -> impl axum::response::IntoResponse {
    ws.on_upgrade(move |ws| async move {
        let ws = WebSocket::new(ws);

        loop {
            ws.send(&SendEvent::FetchNew {
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
            });

            std::thread::sleep(std::time::Duration::from_secs(5));
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
    axum::extract::State(state): axum::extract::State<std::sync::Arc<crate::State>>,
    axum::extract::TypedHeader(cookies): axum::extract::TypedHeader<axum::headers::Cookie>,
    axum::extract::Form(form): axum::extract::Form<PostJoinForm>,
) -> Result<impl axum::response::IntoResponse, axum::response::ErrorResponse> {
    let PostJoinForm { username, room_code } = form;
    let player_id = crate::gen_id();
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
        disconnected: false,
        points: 0,
        streak: 0,
        emoji: crate::EMOJIS[cookies.get("emoji").unwrap().parse::<usize>().unwrap()].to_string(),
        ws: None,
    });
    let player = room.players.last().unwrap();

    // Notify existing players about this newly joined user
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

pub async fn get_room(
    axum::extract::State(state): axum::extract::State<std::sync::Arc<crate::State>>,
    axum::extract::TypedHeader(cookies): axum::extract::TypedHeader<axum::headers::Cookie>,
    axum::extract::Path(room_id): axum::extract::Path<u32>,
) -> Result<impl axum::response::IntoResponse, axum::response::ErrorResponse> {
    let player_id = PlayerId(cookies.get("user").unwrap().parse().unwrap());
    let room = state
        .rooms
        .lock()
        .get(&room_id)
        .ok_or_else(|| axum::response::Redirect::to("/server-browser"))?
        .clone();

    let room = room.lock();
    if !room.players.iter().any(|p| p.id == player_id) {
        return Err(axum::response::Redirect::to(&format!("/join/{}", room_id)).into());
    }

    let html = match room.state {
        RoomState::Lobby => std::fs::read_to_string("frontend/roomLOBBY.html")
            .map_err(|_| axum::http::StatusCode::INTERNAL_SERVER_ERROR)?
            .replace("ROOMID", &room_id.to_string())
            .replace("PLAYERID", &player_id.0.to_string()),
        RoomState::Play { .. } => std::fs::read_to_string("frontend/roomPLAY.html")
            .map_err(|_| axum::http::StatusCode::INTERNAL_SERVER_ERROR)?
            .replace("ROOMID", &room_id.to_string())
            .replace("PLAYERID", &player_id.0.to_string()),
    };
    Ok(axum::response::Html(html))
}

pub async fn get_room_ws(
    axum::extract::State(state): axum::extract::State<std::sync::Arc<crate::State>>,
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

pub async fn fallback(uri: axum::http::Uri) -> impl axum::response::IntoResponse {
    match tokio::fs::read_to_string(format!("frontend{}", uri.path())).await {
        Ok(resp) => Ok(axum::response::Html(resp)),
        Err(_) => {
            Err(axum::response::Redirect::to(&format!("https://guessthesong.io{}", uri.path())))
        }
    }
}
