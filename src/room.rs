use crate::hints::*;
use crate::structs::*;
use crate::utils::*;

async fn play_round(room_arc: std::sync::Arc<parking_lot::Mutex<Room>>) {
    let (round_time, song_title) = {
        let room = room_arc.lock();

        room.send_all(&room.player_state_msg());
        (room.round_time_secs, room.current_song.as_ref().unwrap().title.clone())
    };

    // Pre-generate hints
    let mut hints = Hints::new(&song_title, round_time);

    tokio::time::sleep(std::time::Duration::from_millis(4000)).await; // idk why

    // Start the timer, including countdown
    for timer in (0..=(round_time + 3)).rev() {
        tokio::time::sleep(std::time::Duration::from_millis(1000)).await;
        let mut room = room_arc.lock();

        // Go straight to next round if everyone guessed
        if room.players.iter().all(|p| p.guessed.is_some()) {
            break;
        }

        room.send_all(&SendEvent::Timer {
            message: timer,
            hint: hints.hint_at(timer),
            scores: room.players.iter().map(|p| p.to_player_data()).collect(),
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

        // Show scoreboard
        room.send_all(&SendEvent::Notify { message: format!("The song was: {}", song_title) });
        room.send_all(&SendEvent::Scoreboard {
            round: room.current_round + 1,
            max_rounds: room.num_rounds,
            payload: room.players.iter().map(|p| p.to_scoreboard_player()).collect(),
        });

        // Advance round, stop if this was the last round
        room.current_round += 1;
        if room.current_round == room.num_rounds {
            // somehow stop the game?!
            return;
        }

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

fn points_for_guessing_now(room: &Room) -> u32 {
    let guess_time = (std::time::Instant::now() - room.round_start_time.unwrap()).as_secs_f32();
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

    points
}

fn websocket_event(
    room_arc: &std::sync::Arc<parking_lot::Mutex<Room>>,
    player_id: PlayerId,
    event: ReceiveEvent,
) {
    let mut room = room_arc.lock();

    match event {
        ReceiveEvent::IncomingMsg { msg } => {
            if msg.to_lowercase() == room.current_song.as_ref().unwrap().title.to_lowercase() {
                room.players.iter_mut().find(|p| p.id == player_id).unwrap().guessed =
                    Some(points_for_guessing_now(&room));
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
            // room.send_all(&room.player_state_msg());
            if room.players.iter().all(|p| p.loaded) && room.state == RoomState::WaitingForLoaded {
                room.state = RoomState::Playing;
                room.round_task = Some(spawn_attached(play_round(room_arc.clone())));
            }
        }
        ReceiveEvent::TypingStatus { typing } => {
            room.send_all(&SendEvent::PlayerTyping { uuid: player_id, typing });
        }
        ReceiveEvent::SkipRound => {
            // STUB
        }
        ReceiveEvent::StopGame => {
            // STUB
        }
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
            // Don't have to do anything apparently
        }
    }

    while let Some(event) = ws.recv::<ReceiveEvent>().await {
        websocket_event(&room_arc, player_id, event);
    }

    // Player disconnected
    {
        let mut room = room_arc.lock();

        if let Some(player) = room.players.iter_mut().find(|p| p.id == player_id) {
            player.ws = None;
        }
    }
}
