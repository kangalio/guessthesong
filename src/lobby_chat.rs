pub fn listen(state: std::sync::Arc<crate::State>) {
    let server = std::net::TcpListener::bind("0.0.0.0:9002").unwrap();
    for stream in server.incoming() {
        let mut room_id = None;
        let mut cookies = None;
        let mut websocket = match stream {
            Ok(stream) => match tungstenite::accept_hdr(
                stream,
                |req: &tungstenite::handshake::server::Request, res| {
                    room_id = Some(
                        req.uri()
                            .path()
                            .trim_start_matches("/")
                            .parse::<u32>()
                            .unwrap(),
                    );
                    cookies = Some(req.headers().get("Cookie").unwrap().clone());
                    Ok(res)
                },
            ) {
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
        dbg!(&cookies);
        let user_id = cookies
            .unwrap()
            .to_str()
            .unwrap()
            .split(";")
            .find_map(|s| s.trim().strip_prefix("user="))
            .unwrap()
            .to_string();
        let room_id = room_id.unwrap();

        let username;
        {
            let rooms = state.rooms.lock();
            let room = rooms.iter().find(|r| r.id == room_id).unwrap();
            username = room
                .players
                .iter()
                .find(|p| p.id == user_id)
                .unwrap()
                .name
                .clone();
            let owner_id = room.players.first().map_or("", |p| &p.id);
            std::thread::sleep(std::time::Duration::from_millis(200)); // HACK (to see traffic in firefox)
            websocket
                .write_message(tungstenite::Message::Text(
                    serde_json::to_string(&serde_json::json!( {
                        "state": "joined",
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
                            "owner": owner_id,
                        }
                    } ))
                    .expect("can't fail"),
                ))
                .unwrap();
        }

        let state2 = state.clone();
        std::thread::spawn(move || {
            let state = state2;

            while let Ok(msg) = websocket.read_message() {
                dbg!(&msg);

                #[derive(serde::Deserialize, Debug)]
                #[serde(tag = "type")]
                #[serde(rename_all = "kebab-case")]
                enum Message {
                    IncomingMsg { msg: String },
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
                        websocket
                            .write_message(tungstenite::Message::Text(
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
                            .unwrap();
                    }
                }
            }
        });
    }
}
