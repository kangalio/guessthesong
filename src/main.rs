struct Room {
    name: String,
}

struct State {
    rooms: std::sync::Mutex<Vec<Room>>,
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
            println!("Error: invalid kv pair: {}", kv_pair);
            continue;
        };

        params.insert(key, value);
    }

    state.rooms.lock().unwrap().push(Room {
        name: params.get("room_name").copied().unwrap_or("").to_string(),
    });
}

fn main() {
    let server = tiny_http::Server::http("0.0.0.0:5234").expect("failed to open HTTP server");

    let state = std::sync::Arc::new(State {
        rooms: std::sync::Mutex::new(vec![Room {
            name: "starter room lol".to_string(),
        }]),
    });

    let state2 = state.clone();
    std::thread::spawn(move || {
        let state = state2;

        let server = std::net::TcpListener::bind("0.0.0.0:9001").unwrap();
        for stream in server.incoming() {
            let state2 = state.clone();
            std::thread::spawn(move || {
                let state = state2;
                let mut websocket = tungstenite::accept(stream.unwrap()).unwrap();

                loop {
                    let rooms = state
                        .rooms
                        .lock()
                        .unwrap()
                        .iter()
                        .map(|r| {
                            serde_json::json!( {
                                "code": 1080,
                                "game_mode": "Themes",
                                "idle": 50,
                                "name": &r.name,
                                "players": 4,
                                "status": "Public",
                                "theme": "Random songs",
                            } )
                        })
                        .collect::<Vec<_>>();
                    let msg = serde_json::json!( {
                        "state": "fetch_new",
                        "msg": rooms,
                    } );
                    websocket
                        .write_message(tungstenite::Message::Text(
                            serde_json::to_string(&msg).unwrap(),
                        ))
                        .unwrap();

                    std::thread::sleep(std::time::Duration::from_secs(5));
                }
            });
        }
    });

    loop {
        let mut request = match server.recv() {
            Ok(x) => x,
            Err(e) => {
                println!("Error: failed to receive HTTP request: {}", e);
                continue;
            }
        };
        let mut body = String::new();
        if let Err(e) = request.as_reader().read_to_string(&mut body) {
            println!("Error. failed to read request body: {}", e);
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
                    println!("Error: failed to send HTTP response: {}", e);
                }
            }
            tiny_http::Method::Post => {
                if request.url().contains("create-room") {
                    create_room(&state, &body);
                }
            }
            other => {
                println!("Error: unknown HTTP method: {}", other);
            }
        }
    }
}
