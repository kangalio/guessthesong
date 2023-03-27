struct Player {
    name: String,
}

struct Room {
    name: String,
    id: u32,
    players: Vec<Player>,
    password: Option<String>, // If None, room is public
    explicit_songs: bool,
    num_rounds: u32,
    round_time_secs: u32,
    created_at: std::time::Instant,
}

struct State {
    rooms: parking_lot::Mutex<Vec<Room>>,
}

fn http_url_to_local_path(url: &str) -> std::path::PathBuf {
    let root = std::path::Path::new("/home/kangalioo/dev/rust/guessthesong/frontend/");

    let url = urlencoding::decode(url).unwrap_or_else(|_| url.into());
    let mut path = root.join(url.trim_start_matches('/'));
    if path.extension().is_none() {
        path = path.with_extension("html");
    }

    path
}

fn create_room(state: &State, body: &str) -> tiny_http::Response<std::io::Empty> {
    let mut params = std::collections::HashMap::new();
    for kv_pair in body.split('&') {
        let Some((key, value)) = kv_pair.split_once('=') else {
            log::error!("invalid kv pair: {}", kv_pair);
            continue;
        };

        params.insert(key, value);
    }

    let username = params.get("username").copied().unwrap_or("");
    let id;
    {
        let mut rooms = state.rooms.lock();
        id = rooms.iter().map(|r| r.id).max().unwrap_or(0) + 1; // generate a new unambiguous ID
        rooms.push(Room {
            name: params.get("room_name").copied().unwrap_or("").to_string(),
            id,
            password: match params.get("password").copied() {
                None | Some("") => None,
                Some(x) => Some(x.to_string()),
            },
            players: vec![Player {
                name: username.to_string(),
            }],
            explicit_songs: params.get("explicit").copied() == Some("y"),
            num_rounds: params
                .get("rounds")
                .and_then(|x| x.parse().ok())
                .unwrap_or(9),
            round_time_secs: params
                .get("round_time")
                .and_then(|x| x.parse().ok())
                .unwrap_or(75),
            created_at: std::time::Instant::now(),
        });
    }

    // TODO: don't hardcode the URL somehow
    tiny_http::Response::empty(302)
        .with_header(
            tiny_http::Header::from_bytes("Location", format!("http://127.0.0.1:5234/room/{}", id))
                .expect("cant happen"),
        )
        .with_header(
            tiny_http::Header::from_bytes("Set-Cookie", format!("user={}", username))
                .expect("cant happen"),
        )
}

fn join_room(state: &State, body: &str) -> tiny_http::Response<std::io::Empty> {
    let mut params = std::collections::HashMap::new();
    for kv_pair in body.split('&') {
        let Some((key, value)) = kv_pair.split_once('=') else {
            log::error!("invalid kv pair: {}", kv_pair);
            continue;
        };

        params.insert(key, value);
    }

    let username = params.get("username").copied().unwrap_or("");
    let room_id = params
        .get("room_code")
        .copied()
        .unwrap()
        .parse::<u32>()
        .unwrap();

    {
        let mut rooms = state.rooms.lock();
        let room = rooms.iter_mut().find(|r| r.id == room_id).unwrap();
        room.players.push(Player {
            name: username.to_string(),
        })
    }

    // TODO: don't hardcode the URL somehow
    tiny_http::Response::empty(302)
        .with_header(
            tiny_http::Header::from_bytes(
                "Location",
                format!("http://127.0.0.1:5234/room/{}", room_id),
            )
            .expect("cant happen"),
        )
        .with_header(
            tiny_http::Header::from_bytes("Set-Cookie", format!("user={}", username))
                .expect("cant happen"),
        )
}

fn main() {
    env_logger::init();

    let server = tiny_http::Server::http("0.0.0.0:5234").expect("failed to open HTTP server");

    let state = std::sync::Arc::new(State {
        rooms: parking_lot::Mutex::new(vec![Room {
            id: 420,
            name: "starter room lol".to_string(),
            players: vec![],
            password: None,
            explicit_songs: true,
            num_rounds: 9,
            round_time_secs: 75,
            created_at: std::time::Instant::now(),
        }]),
    });

    let state2 = state.clone();
    std::thread::spawn(move || {
        let state = state2;

        let server = std::net::TcpListener::bind("0.0.0.0:9001").unwrap();
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
                            "Failed to send room data, stopping WebSocket connection: {}",
                            e
                        );
                        break;
                    }

                    std::thread::sleep(std::time::Duration::from_secs(5));
                }
            });
        }
    });

    let state2 = state.clone();
    std::thread::spawn(move || {
        let state = state2;

        let server = std::net::TcpListener::bind("0.0.0.0:9002").unwrap();
        for stream in server.incoming() {
            let mut cookies = None;
            let mut websocket = match stream {
                Ok(stream) => match tungstenite::accept_hdr(
                    stream,
                    |req: &tungstenite::handshake::server::Request, res| {
                        cookies = req.headers().get("Cookie").cloned();
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
            let username = cookies
                .unwrap()
                .to_str()
                .unwrap()
                .split(";")
                .find_map(|s| s.trim().strip_prefix("user="))
                .unwrap()
                .to_string();

            websocket
                .write_message(tungstenite::Message::Text(
                    serde_json::to_string(&serde_json::json!( {
                        "state": "joined",
                        "payload": {
                            "state": "player_data",
                            "payload": [
                                {
                                    "uuid": "eb0496f2-a8b7-49d2-bdc4-9727e7969aa0",
                                    "username": username,
                                    "points": 0,
                                    "streak": 0,
                                    "emoji": "😝",
                                    "prev_points": 0,
                                    "loaded": false,
                                    "guessed": false,
                                    "disconnected": false,
                                    "game_state": "Lobby"
                                }
                            ],
                            "owner": "eb0496f2-a8b7-49d2-bdc4-9727e7969aa0"
                        }
                    } ))
                    .expect("can't fail"),
                ))
                .unwrap();

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
                                        "uuid": "eb0496f2-a8b7-49d2-bdc4-9727e7969aa0",
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
    });

    loop {
        let mut request = match server.recv() {
            Ok(x) => x,
            Err(e) => {
                log::error!("failed to receive HTTP request: {}", e);
                continue;
            }
        };
        let mut body = String::new();
        if let Err(e) = request.as_reader().read_to_string(&mut body) {
            log::error!("failed to read request body: {}", e);
        }

        match request.method() {
            tiny_http::Method::Get => {
                let response_result = if let Some((_, room_id)) = request.url().split_once("/room/")
                {
                    match std::fs::read_to_string("frontend/room.html") {
                        Ok(html) => {
                            let html = html.replace("ROOMID", room_id); // TODO: .replace("USERID");
                            request.respond(
                                tiny_http::Response::from_data(html).with_header(
                                    tiny_http::Header::from_bytes("Content-Type", "text/html")
                                        .expect("can't fail"),
                                ),
                            )
                        }
                        Err(e) => request.respond(
                            tiny_http::Response::from_string(format!("{}", e))
                                .with_status_code(404),
                        ),
                    }
                } else if let Some((_, room_id)) = request.url().split_once("/join/") {
                    match std::fs::read_to_string("frontend/join.html") {
                        Ok(html) => {
                            let html = html.replace("ROOMID", room_id);
                            request.respond(
                                tiny_http::Response::from_data(html).with_header(
                                    tiny_http::Header::from_bytes("Content-Type", "text/html")
                                        .expect("can't fail"),
                                ),
                            )
                        }
                        Err(e) => request.respond(
                            tiny_http::Response::from_string(format!("{}", e))
                                .with_status_code(404),
                        ),
                    }
                } else if let Ok(file) = std::fs::File::open(http_url_to_local_path(request.url()))
                {
                    request.respond(tiny_http::Response::from_file(file))
                } else {
                    let redirect_target = format!("https://guessthesong.io/{}", request.url());
                    request.respond(tiny_http::Response::empty(302).with_header(
                        tiny_http::Header::from_bytes("Location", redirect_target).unwrap(),
                    ))
                };

                if let Err(e) = response_result {
                    log::error!("failed to send HTTP response: {}", e);
                }
            }
            tiny_http::Method::Post => {
                if request.url().contains("create-room") {
                    if let Err(e) = request.respond(create_room(&state, &body)) {
                        log::error!("failed to send HTTP response: {}", e);
                    }
                } else if request.url().contains("join") {
                    if let Err(e) = request.respond(join_room(&state, &body)) {
                        log::error!("failed to send HTTP response: {}", e);
                    }
                }
            }
            other => {
                log::error!("unknown HTTP method: {}", other);
            }
        }
    }
}
