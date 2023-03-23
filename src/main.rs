struct Player {
    name: String,
}

struct Room {
    name: String,
    code: u32,
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

fn create_room(state: &State, body: &str) {
    let mut params = std::collections::HashMap::new();
    for kv_pair in body.split('&') {
        let Some((key, value)) = kv_pair.split_once('=') else {
            log::error!("invalid kv pair: {}", kv_pair);
            continue;
        };

        params.insert(key, value);
    }

    {
        let mut rooms = state.rooms.lock();
        let code = rooms.iter().map(|r| r.code).max().unwrap_or(0) + 1; // generate a new unambiguous code
        rooms.push(Room {
            name: params.get("room_name").copied().unwrap_or("").to_string(),
            code,
            password: match params.get("password").copied() {
                None | Some("") => None,
                Some(x) => Some(x.to_string()),
            },
            players: vec![Player {
                name: params.get("username").copied().unwrap_or("").to_string(),
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
}

fn main() {
    env_logger::init();

    let server = tiny_http::Server::http("0.0.0.0:5234").expect("failed to open HTTP server");

    let state = std::sync::Arc::new(State {
        rooms: parking_lot::Mutex::new(vec![Room {
            code: 420,
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
                                "code": r.code,
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
                let response_result =
                    match std::fs::File::open(http_url_to_local_path(request.url())) {
                        Ok(file) => request.respond(tiny_http::Response::from_file(file)),
                        Err(e) => request.respond(
                            tiny_http::Response::from_string(format!("{}", e))
                                .with_status_code(404),
                        ),
                    };
                if let Err(e) = response_result {
                    log::error!("failed to send HTTP response: {}", e);
                }
            }
            tiny_http::Method::Post => {
                if request.url().contains("create-room") {
                    create_room(&state, &body);
                }
            }
            other => {
                log::error!("unknown HTTP method: {}", other);
            }
        }
    }
}
