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

        room_state = room.state;
    };

    let html = match room_state {
        crate::RoomState::Lobby => std::fs::read_to_string("frontend/roomLOBBY.html")
            .map_err(|_| tiny_http::Response::empty(500))?
            .replace("ROOMID", &room_id_str)
            .replace("USERID", user_id),
        crate::RoomState::Play => std::fs::read_to_string("frontend/roomPLAY.html")
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
    let (mut ws_write, mut ws_read) = websocket.split();

    let (player_websockets, msg);
    {
        let mut rooms = state.rooms.lock();
        let room = rooms
            .iter_mut()
            .find(|r| r.id == room_id)
            .expect("room was deleted inbetween websocket connection accept and first message");
        room.players
            .iter_mut()
            .find(|p| p.id == user_id)
            .unwrap()
            .websocket = Some(std::sync::Arc::new(tokio::sync::Mutex::new(ws_write)));

        player_websockets = room
            .players
            .iter()
            .filter_map(|p| p.websocket.clone())
            .collect::<Vec<_>>();
        msg = tungstenite::Message::Text(
            serde_json::to_string(&serde_json::json!( {
                "state": "join",
                "message": username, // Yes this does something
                "payload": {
                    "state": "player_data",
                    "payload": room.players.iter().map(|player| serde_json::json!( {
                        "uuid": player.id,
                        "username": player.name,
                        "points": 0,
                        "streak": 0,
                        "emoji": "😝",
                        "prev_points": 0,
                        "loaded": false,
                        "guessed": false,
                        "disconnected": false,
                        "game_state": "Lobby"
                    } )).collect::<Vec<_>>(),
                    "owner": room.players.first().map_or("", |p| &p.id).to_string(),
                }
            } ))
            .expect("can't fail"),
        );
    };

    std::thread::sleep(std::time::Duration::from_millis(200)); // HACK (to see traffic in firefox)
    for websocket in player_websockets {
        websocket.lock().await.send(msg.clone()).await.unwrap();
    }

    while let Some(Ok(msg)) = ws_read.next().await {
        dbg!(&msg);

        #[derive(serde::Deserialize, Debug)]
        #[serde(tag = "type")]
        #[serde(rename_all = "kebab-case")]
        enum Message {
            IncomingMsg { msg: String },
            StartGame,
        }

        let msg: Message = match msg {
            tungstenite::Message::Text(msg) => match serde_json::from_str(&msg) {
                Ok(msg) => msg,
                Err(e) => {
                    log::error!("malformed websocket message {:?}: {}", e, msg);
                    continue;
                }
            },
            other => {
                log::info!("ignoring websocket message {:?}", other);
                continue;
            }
        };

        dbg!(&msg);
        match msg {
            Message::IncomingMsg { msg } => {
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
                    if let Err(e) = websocket
                        .lock()
                        .await
                        .send(tungstenite::Message::Text(
                            serde_json::to_string(&serde_json::json!( {
                                "type": "message",
                                "state": "chat",
                                "username": username,
                                "uuid": user_id,
                                "msg": msg,
                                "time_stamp": "Mar-25 09:42PM",
                            } ))
                            .expect("can't fail"),
                        ))
                        .await
                    {
                        log::error!("can't send websocket message: {}", e);
                        break;
                    }
                }
            }
            Message::StartGame => {
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
                    websocket
                        .lock()
                        .await
                        .send(tungstenite::Message::Text(
                            serde_json::to_string(&serde_json::json!( {
                                "state": "start_game",
                            } ))
                            .unwrap(),
                        ))
                        .await
                        .unwrap();
                }

                {
                    let mut rooms = state.rooms.lock();
                    let room = rooms.iter_mut().find(|r| r.id == room_id).unwrap();
                    room.state = crate::RoomState::Play;
                    for player in &mut room.players {
                        player.websocket = None;
                    }
                }
            }
        }
    }
}

async fn ingame_ws(
    state: &crate::State,
    websocket: tokio_tungstenite::WebSocketStream<tokio::net::TcpStream>,
    req_data: InitialRequest,
) {
    let InitialRequest {
        room_id,
        user_id,
        username,
    } = req_data;
    let (mut ws_write, mut ws_read) = websocket.split();

    let player_websockets;
    let msg = {
        let mut rooms = state.rooms.lock();
        let room = rooms
            .iter_mut()
            .find(|r| r.id == room_id)
            .expect("room was deleted inbetween websocket connection accept and first message");
        player_websockets = room
            .players
            .iter()
            .filter_map(|p| p.websocket.clone())
            .collect::<Vec<_>>();
        tungstenite::Message::Text(
            serde_json::to_string(&serde_json::json!( {
                "state": "join",
                "message": username, // Yes this does something
                "payload": {
                    "state": "player_data",
                    "payload": room.players.iter().map(|player| serde_json::json!( {
                        "uuid": player.id,
                        "username": player.name,
                        "points": 0,
                        "streak": 0,
                        "emoji": "😝",
                        "prev_points": 0,
                        "loaded": false,
                        "guessed": false,
                        "disconnected": false,
                        "game_state": "Lobby"
                    } )).collect::<Vec<_>>(),
                    "owner": room.players.first().map_or("", |p| &p.id).to_string(),
                }
            } ))
            .expect("can't fail"),
        )
    };

    std::thread::sleep(std::time::Duration::from_millis(200)); // HACK (to see traffic in firefox)
    ws_write.send(msg).await.unwrap();

    let msg = tungstenite::Message::Text(
        serde_json::to_string(&serde_json::json!( {
                "state": "new_turn",
            } ))
        .expect("can't fail"),
    );
    ws_write.send(msg).await.unwrap();

    let msg = tungstenite::Message::Text(
        serde_json::to_string(&serde_json::json!( {
                "state": "loading",
            } ))
        .expect("can't fail"),
    );
    ws_write.send(msg).await.unwrap();

    {
        let mut rooms = state.rooms.lock();
        let room = rooms.iter_mut().find(|r| r.id == room_id).unwrap();
        room.players
            .iter_mut()
            .find(|p| p.id == user_id)
            .unwrap()
            .websocket = Some(std::sync::Arc::new(tokio::sync::Mutex::new(ws_write)));
    }

    while let Some(Ok(msg)) = ws_read.next().await {
        dbg!(&msg);

        #[derive(serde::Deserialize, Debug)]
        #[serde(tag = "type")]
        #[serde(rename_all = "kebab-case")]
        enum Message {
            IncomingMsg { msg: String },
            TypingStatus { typing: bool },
            AudioLoaded,
        }

        let msg: Message = match msg {
            tungstenite::Message::Text(msg) => match serde_json::from_str(&msg) {
                Ok(msg) => msg,
                Err(e) => {
                    log::error!("malformed websocket message {:?}: {}", e, msg);
                    continue;
                }
            },
            other => {
                log::info!("ignoring websocket message {:?}", other);
                continue;
            }
        };

        dbg!(&msg);
        match msg {
            Message::IncomingMsg { msg } => {
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
                    if let Err(e) = websocket
                        .lock()
                        .await
                        .send(tungstenite::Message::Text(
                            serde_json::to_string(&serde_json::json!( {
                                "type": "message",
                                "state": "chat",
                                "username": username,
                                "uuid": user_id,
                                "msg": msg,
                                "time_stamp": "Mar-25 09:42PM",
                            } ))
                            .expect("can't fail"),
                        ))
                        .await
                    {
                        log::error!("can't send websocket message: {}", e);
                        break;
                    }
                }
            }
            Message::TypingStatus { typing: _ } => {
                // STUB
            }
            Message::AudioLoaded => {
                // STUB
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

        let room_state = state
            .rooms
            .lock()
            .iter()
            .find(|r| r.id == req_data.room_id)
            .unwrap()
            .state;
        let state2 = state.clone();
        tokio::spawn(async move {
            match room_state {
                crate::RoomState::Lobby => lobby_ws(&state2, ws, req_data).await,
                crate::RoomState::Play => ingame_ws(&state2, ws, req_data).await,
            }
        });
    }
}
