use crate::*;

pub async fn get_server_browser(
    axum::extract::State(state): axum::extract::State<std::sync::Arc<crate::State>>,
    ws: axum::extract::WebSocketUpgrade,
) -> impl axum::response::IntoResponse {
    ws.on_upgrade(move |ws| async move {
        let ws = WebSocket::new(ws);

        loop {
            let msg = SendEvent::FetchNew {
                msg: state
                    .rooms
                    .lock()
                    .iter()
                    .map(|room| ListedRoom {
                        code: room.meta.id,
                        idle: (std::time::Instant::now() - room.meta.created_at).as_secs(),
                        name: room.meta.name.clone(),
                        players: room.meta.player_ids.len(),
                        status: if room.meta.password.is_some() {
                            ListedRoomState::Private
                        } else {
                            ListedRoomState::Public
                        },
                        theme: room.meta.theme.clone(),
                        game_mode: "Themes".into(), // is this ever something else?
                    })
                    .collect(),
            };
            ws.send(&msg).await;

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

    {
        let mut rooms = state.rooms.lock();
        let room = rooms
            .iter_mut()
            .find(|room| room.meta.id == form.room_code)
            .ok_or_else(|| axum::response::Redirect::to("/server-browser"))?;

        let join_event =
            crate::room_runner::RoomRunnerMessage::PlayerJoin(crate::room_runner::Player {
                name: username.to_string(),
                id: player_id,
                loaded: false,
                guessed: None,
                disconnected: false,
                points: 0,
                streak: 0,
                emoji: crate::EMOJIS[cookies.get("emoji").unwrap().parse::<usize>().unwrap()]
                    .to_string(),
                ws: None,
            });
        room.runner_tx.send(join_event).unwrap();
    }

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
    let user_id = cookies.get("user").unwrap().to_string();

    let room_state;
    {
        let rooms = state.rooms.lock();
        let room = rooms
            .iter()
            .find(|r| r.meta.id == room_id)
            .ok_or_else(|| axum::response::Redirect::to("/server-browser"))?;

        if !room.meta.player_ids.lock().contains(&user_id) {
            return Err(axum::response::Redirect::to(&format!("/join/{}", room_id)).into());
        }

        room_state = room.meta.state.clone();
    };

    let html = match room_state {
        crate::RoomState::Lobby => std::fs::read_to_string("frontend/roomLOBBY.html")
            .map_err(|_| axum::http::StatusCode::INTERNAL_SERVER_ERROR)?
            .replace("ROOMID", &room_id.to_string())
            .replace("USERID", &user_id),
        crate::RoomState::Play { .. } => std::fs::read_to_string("frontend/roomPLAY.html")
            .map_err(|_| axum::http::StatusCode::INTERNAL_SERVER_ERROR)?
            .replace("ROOMID", &room_id.to_string())
            .replace("PLAYERID", &user_id),
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

    ws.on_upgrade(move |ws| async move {
        let mut rooms = state.rooms.lock();
        let Some(room) = rooms.iter_mut().find(|room| room.meta.id == room_id) else { return };
        let join_event = crate::room_runner::RoomRunnerMessage::WebsocketConnect { ws, player_id };
        room.runner_tx.send(join_event).unwrap();
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
