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

#[derive(Clone)]
pub enum RoomState {
    Lobby,
    Play,
}

pub struct Room {
    pub name: String,
    pub players: Vec<Player>,
    pub password: Option<String>, // If None, room is public
    // explicit_songs: bool,
    pub num_rounds: u32,
    pub round_time_secs: u32,
    pub created_at: std::time::Instant,
    pub state: RoomState,
    pub theme: String,
}

impl Room {
    pub fn send_all(&self, msg: &SendEvent) {
        for player in &self.players {
            if let Some(ws) = &player.ws {
                ws.send(msg);
            }
        }
    }

    pub fn player_state_msg(&self) -> SendEvent {
        SendEvent::PlayerData {
            payload: self
                .players
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
                .collect(),
            owner: self.players.first().unwrap().id,
        }
    }
}

pub async fn websocket_connect(
    room: std::sync::Arc<parking_lot::Mutex<Room>>,
    player_id: PlayerId,
    ws: std::sync::Arc<WebSocket>,
) {
    {
        let mut room = room.lock();

        // Notify newly joined players about all existing players
        for player in &room.players {
            ws.send(&SendEvent::Join {
                message: player.name.clone(),
                payload: Box::new(room.player_state_msg()),
            });
        }

        if let Some(player) = room.players.iter_mut().find(|p| p.id == player_id) {
            player.ws = Some(ws.clone());
        }
    }

    while let Some(event) = ws.recv::<ReceiveEvent>().await {
        let room = room.lock();

        let player = room.players.iter().find(|p| p.id == player_id).unwrap();
        match event {
            ReceiveEvent::IncomingMsg { msg } => room.send_all(&SendEvent::Chat {
                r#type: "message".into(),
                uuid: player.id,
                username: player.name.clone(),
                msg,
            }),
            ReceiveEvent::AudioLoad => todo!(),
            ReceiveEvent::TypingStatus { typing } => todo!(),
        }
    }

    {
        let mut room = room.lock();

        if let Some(player) = room.players.iter_mut().find(|p| p.id == player_id) {
            player.ws = None;
        }
    }
}
