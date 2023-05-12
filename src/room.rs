use crate::*;

#[derive(Debug)]
pub struct Player {
    /// This must be an Arc to clone it out and avoid locking the room data while waiting for a
    /// receive event
    pub ws: Option<std::sync::Arc<WebSocket>>,
    pub name: String,
    pub id: PlayerId,
    pub loaded: bool,
    pub guessed: Option<u32>, // Points gained
    pub streak: u32,
    pub points: u32,
    pub emoji: String,
    pub disconnected: bool,
}

#[derive(Clone, PartialEq, Eq)]
pub enum RoomState {
    Lobby,
    WaitingForReconnect,
    WaitingForLoaded,
    Playing,
}

pub struct Room {
    // Static data
    pub name: String,
    pub password: Option<String>, // If None, room is public
    // explicit_songs: bool,
    pub num_rounds: u32,
    pub round_time_secs: u32,
    pub created_at: std::time::Instant,
    pub theme: String,

    pub song_provider: std::sync::Arc<SongProvider>,
    pub players: Vec<Player>,
    pub current_round: u32, // zero-indexed
    pub state: RoomState,
    pub round_task: Option<AttachedTask>,
    pub current_song: Option<Song>,
    pub round_start_time: Option<std::time::Instant>,
}

impl Room {
    pub fn send_all(&self, msg: &SendEvent) {
        for player in &self.players {
            if let Some(ws) = &player.ws {
                ws.send(msg);
            }
        }
    }

    pub fn player_datas(&self) -> Vec<SinglePlayerData> {
        self.players
            .iter()
            .map(|p| SinglePlayerData {
                uuid: p.id,
                username: p.name.clone(),
                points: p.points,
                prev_points: p.points - p.guessed.unwrap_or(0),
                streak: p.streak,
                emoji: p.emoji.clone(),
                loaded: p.loaded,
                guessed: p.guessed.is_some(),
                disconnected: p.disconnected,
            })
            .collect()
    }

    pub fn player_state_msg(&self) -> SendEvent {
        SendEvent::PlayerData {
            payload: self.player_datas(),
            owner: self.players.first().unwrap().id,
        }
    }
}

fn generate_hints(title: &str, num_steps: usize) -> (String, Vec<String>) {
    fn blank_out_indices(s: &str, indices: &[usize]) -> String {
        s.chars().enumerate().map(|(i, c)| if indices.contains(&i) { '_' } else { c }).collect()
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

async fn play_round(room_arc: std::sync::Arc<parking_lot::Mutex<Room>>) {
    let (round_time, song_title) = {
        let room = room_arc.lock();

        room.send_all(&room.player_state_msg());
        (room.round_time_secs, room.current_song.as_ref().unwrap().title.clone())
    };

    // Pre-generate hints
    let hints_at = (10..u32::min(round_time, 70)).step_by(10).rev();
    let (mut current_hint, hints) = generate_hints(&song_title, hints_at.len());
    let mut hints_at = hints_at.zip(hints).collect::<std::collections::HashMap<_, _>>();

    // Start the timer, including countdown
    for timer in (0..=(round_time + 3)).rev() {
        tokio::time::sleep(std::time::Duration::from_millis(1000)).await;
        let mut room = room_arc.lock();

        // Go straight to next round if everyone guessed
        if room.players.iter().all(|p| p.guessed.is_some()) {
            break;
        }

        // Update hint
        if let Some(new_hint) = hints_at.remove(&timer) {
            current_hint = new_hint;
        }
        room.send_all(&SendEvent::Timer {
            message: timer,
            hint: current_hint.clone(),
            scores: room.player_datas(),
            round_time,
        });

        // Log round start time (needed for point calculation later)
        if timer == round_time {
            room.round_start_time = Some(std::time::Instant::now());
        }
    }

    let song_provider = {
        let mut room = room_arc.lock();

        // Add up points and streak
        for p in &mut room.players {
            if let Some(new_points) = p.guessed {
                p.points += new_points;
                p.streak += 1;
            } else {
                p.streak = 0;
            }
        }

        // Advance round, stop if this was the last round
        room.current_round += 1;
        if room.current_round == room.num_rounds {
            // somehow stop the game?!
            return;
        }

        // Show scoreboard
        room.send_all(&SendEvent::Notify { message: format!("The song was: {}", song_title) });
        room.send_all(&SendEvent::Scoreboard {
            round: room.current_round,
            max_rounds: room.num_rounds,
            payload: room
                .players
                .iter()
                .map(|p| ScoreboardPlayer {
                    uuid: p.id,
                    display_name: p.name.clone(),
                    points: p.points,
                    points_diff: p.guessed.unwrap_or(0),
                    prev_points: p.points - p.guessed.unwrap_or(0),
                    streak: p.streak,
                })
                .collect(),
        });

        room.song_provider.clone()
    };

    // Give users time to look at scoreboard
    // tokio::time::sleep(std::time::Duration::from_secs(3)).await;
    let new_song = song_provider.next().await;

    // Kick off next round
    {
        let mut room = room_arc.lock();

        room.current_song = Some(new_song);
        for p in &mut room.players {
            p.guessed = None;
            p.loaded = false;
        }
        room.send_all(&SendEvent::NewTurn);
        room.state = RoomState::WaitingForLoaded;
    }
}

pub async fn websocket_connect(
    room_arc: std::sync::Arc<parking_lot::Mutex<Room>>,
    player_id: PlayerId,
    ws: std::sync::Arc<WebSocket>,
) {
    tokio::time::sleep(std::time::Duration::from_millis(100)).await; // Hack to see traffic in firefox dev tools

    let room_state = {
        let mut room = room_arc.lock();

        if let Some(player) = room.players.iter_mut().find(|p| p.id == player_id) {
            player.ws = Some(ws.clone());
        }
        room.state.clone()
    };
    match room_state {
        RoomState::Lobby => {
            let room = room_arc.lock();

            // Notify newly joined players about all existing players
            for player in &room.players {
                ws.send(&SendEvent::Join {
                    message: player.name.clone(),
                    payload: Box::new(room.player_state_msg()),
                });
            }
        }
        RoomState::WaitingForReconnect => {
            let (everyone_connected, song_provider) = {
                let room = room_arc.lock();

                (room.players.iter().all(|p| p.ws.is_some()), room.song_provider.clone())
            };

            if everyone_connected {
                let song = song_provider.next().await;

                let mut room = room_arc.lock();

                room.current_song = Some(song);
                room.send_all(&SendEvent::NewTurn);
                room.send_all(&SendEvent::Loading);

                room.state = RoomState::WaitingForLoaded;
            }
        }
        RoomState::WaitingForLoaded => {
            unimplemented!();
        }
        RoomState::Playing => {
            unimplemented!();
        }
    }

    while let Some(event) = ws.recv::<ReceiveEvent>().await {
        let mut room = room_arc.lock();

        match event {
            ReceiveEvent::IncomingMsg { msg } => {
                if msg.to_lowercase() == room.current_song.as_ref().unwrap().title.to_lowercase() {
                    let guess_time =
                        (std::time::Instant::now() - room.round_start_time.unwrap()).as_secs_f32();
                    let how_many_others_have_already_guessed =
                        room.players.iter().filter(|p| p.guessed.is_some()).count();
                    let hints_left = ((room.round_time_secs as f32 - guess_time) / 10.0) as u32;

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

                    room.players.iter_mut().find(|p| p.id == player_id).unwrap().guessed =
                        Some(points);
                } else {
                    let player = room.players.iter().find(|p| p.id == player_id).unwrap();
                    room.send_all(&SendEvent::Chat {
                        r#type: "message".into(),
                        uuid: player.id,
                        username: player.name.clone(),
                        msg,
                    })
                }
            }
            ReceiveEvent::StartGame => {
                room.send_all(&SendEvent::StartGame);
                room.state = RoomState::WaitingForReconnect;
                // Everyone will reconnect now
            }
            ReceiveEvent::AudioLoaded => {
                room.players.iter_mut().find(|p| p.id == player_id).unwrap().loaded = true;
                if room.players.iter().all(|p| p.loaded)
                    && room.state == RoomState::WaitingForLoaded
                {
                    room.state = RoomState::Playing;
                    room.round_task = Some(spawn_attached(play_round(room_arc.clone())));
                }
            }
            ReceiveEvent::TypingStatus { typing } => {
                room.send_all(&SendEvent::PlayerTyping { uuid: player_id, typing });
            }
        }
    }

    // Player disconnected
    {
        let mut room = room_arc.lock();

        if let Some(player) = room.players.iter_mut().find(|p| p.id == player_id) {
            player.ws = None;
        }
    }
}
