pub fn listen(state: std::sync::Arc<crate::State>) {
    let server =
        std::net::TcpListener::bind("0.0.0.0:9001").expect("fatal: can't open websocket server");
    for stream in server.incoming() {
        let mut websocket = match stream {
            Ok(stream) => match tungstenite::accept(stream) {
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

        let state2 = state.clone();
        std::thread::spawn(move || {
            let state = state2;

            loop {
                let rooms = state
                    .rooms
                    .lock()
                    .iter()
                    .map(|r| {
                        serde_json::json!( {
                                "code": r.id,
                                "game_mode": "Themes",
                                "idle": (std::time::Instant::now() - r.created_at).as_secs(),
                                "name": &r.name,
                                "players": r.players.len(),
                                "status": if r.password.is_some() { "Private" } else { "Public" },
                                "theme": "Random songs",
                            } )
                    })
                    .collect::<Vec<_>>();
                let msg = serde_json::json!( {
                        "state": "fetch_new",
                        "msg": rooms,
                    } );

                if let Err(e) = websocket.write_message(tungstenite::Message::Text(
                    serde_json::to_string(&msg).expect("can't happen"),
                )) {
                    log::info!(
                        "Stopping WebSocket connection: Failed to send room data: {}",
                        e
                    );
                    break;
                }

                std::thread::sleep(std::time::Duration::from_secs(5));
            }
        });
    }
}
