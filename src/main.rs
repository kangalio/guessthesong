use std::collections::HashMap;
mod browse_rooms;
mod lobby_chat;

struct Player {
    name: String,
    id: String,
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
    // No owner ID here: the first player is automatically owner
}

pub struct State {
    rooms: parking_lot::Mutex<Vec<Room>>,
}

fn extract_user_id_cookie(cookie_header: &str) -> Result<&str, &'static str> {
    cookie_header
        .split(";")
        .find_map(|s| s.trim().strip_prefix("user="))
        .ok_or_else(|| "missing user cookie")
}

fn gen_id() -> String {
    thread_local! {
        static START_TIME: std::time::Instant = std::time::Instant::now();
    }
    START_TIME
        .with(|&start_time| std::time::Instant::now() - start_time)
        .as_nanos()
        .to_string()
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

fn parse_formdata(body: &str) -> HashMap<&str, &str> {
    let mut params = HashMap::new();

    for kv_pair in body.split('&') {
        let Some((key, value)) = kv_pair.split_once('=') else {
            log::error!("invalid kv pair: {}", kv_pair);
            continue;
        };

        params.insert(key, value);
    }

    params
}

fn create_room(state: &State, body: &str) -> tiny_http::Response<std::io::Empty> {
    let params = parse_formdata(body);
    let username = params.get("username").copied().unwrap_or("");
    let user_id = gen_id();
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
                id: user_id.clone(),
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
            tiny_http::Header::from_bytes("Set-Cookie", format!("user={}; Path=/", user_id))
                .expect("cant happen"),
        )
}

fn redirect(target_url: &str) -> tiny_http::Response<std::io::Empty> {
    tiny_http::Response::empty(302)
        .with_header(tiny_http::Header::from_bytes("Location", target_url).expect("can't happen"))
}

fn join_room(
    state: &State,
    body: &str,
) -> Result<tiny_http::Response<std::io::Empty>, tiny_http::Response<std::io::Empty>> {
    let params = parse_formdata(body);
    let username = params
        .get("username")
        .ok_or_else(|| tiny_http::Response::empty(400))?;
    let user_id = gen_id();
    let room_id = params
        .get("room_code")
        .copied()
        .ok_or_else(|| tiny_http::Response::empty(400))?
        .parse::<u32>()
        .map_err(|_| tiny_http::Response::empty(400))?;

    {
        let mut rooms = state.rooms.lock();
        let room = rooms
            .iter_mut()
            .find(|r| r.id == room_id)
            .ok_or_else(|| redirect("http://127.0.0.1:5234/server-browser"))?;
        room.players.push(Player {
            name: username.to_string(),
            id: user_id.clone(),
        })
    }

    // TODO: don't hardcode the URL somehow
    Ok(tiny_http::Response::empty(302)
        .with_header(
            tiny_http::Header::from_bytes(
                "Location",
                format!("http://127.0.0.1:5234/room/{}", room_id),
            )
            .expect("cant happen"),
        )
        .with_header(
            tiny_http::Header::from_bytes("Set-Cookie", format!("user={}; Path=/", user_id))
                .expect("cant happen"),
        ))
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
        browse_rooms::listen(state2);
    });

    let state2 = state.clone();
    std::thread::spawn(move || {
        tokio::runtime::Runtime::new()
            .unwrap()
            .block_on(lobby_chat::listen(state2));
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
                let response_result = if let Some((_, room_id_str)) =
                    request.url().split_once("/room/")
                {
                    match lobby_chat::get_room(&state, &request, room_id_str) {
                        Ok(resp) => request.respond(resp),
                        Err(resp) => request.respond(resp),
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
                } else if request.url().contains("/static") {
                    let redirect_target = format!("https://guessthesong.io/{}", request.url());
                    request.respond(redirect(&redirect_target))
                } else {
                    request.respond(tiny_http::Response::empty(404))
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
                    let response = match join_room(&state, &body) {
                        Ok(x) => x,
                        Err(x) => x,
                    };
                    if let Err(e) = request.respond(response) {
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
