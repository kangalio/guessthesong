use crate::song_provider::*;
use crate::utils::*;
use crate::ws_structs::*;

#[derive(Debug)]
pub struct Player {
    /// This must be an Arc to clone it out and avoid locking the room data while waiting for a
    /// receive event
    pub ws: parking_lot::Mutex<Option<std::sync::Arc<WebSocket>>>,
    pub name: String,
    pub id: PlayerId,
    pub loaded: bool,
    pub guessed: Option<u32>, // Points gained
    pub streak: u32,
    pub points: u32,
    pub emoji: String,
}

impl Player {
    /// Wraps WebSocket::send, marks player as disconnected if send fails
    pub fn send(&self, msg: &SendEvent) {
        let mut ws_maybe = self.ws.lock();
        if let Some(ws) = &*ws_maybe {
            if let Err(()) = ws.send(msg) {
                *ws_maybe = None;
            }
        }
    }
}

impl Player {
    pub fn to_scoreboard_player(&self) -> ScoreboardPlayer {
        ScoreboardPlayer {
            uuid: self.id,
            display_name: self.name.clone(),
            points: self.points,
            point_diff: self.guessed.unwrap_or(0),
            streak: self.streak,
        }
    }

    pub fn to_player_data(&self) -> SinglePlayerData {
        SinglePlayerData {
            uuid: self.id,
            username: self.name.clone(),
            points: self.points + self.guessed.unwrap_or(0),
            prev_points: self.points,
            streak: self.streak,
            emoji: self.emoji.clone(),
            loaded: self.loaded,
            guessed: self.guessed.is_some(),
            disconnected: self.ws.lock().is_none(),
        }
    }
}

#[derive(Clone, PartialEq, Eq)]
pub enum RoomState {
    Lobby,
    WaitingForReconnect,
    WaitingForLoaded,
    RoundStarted,
}

pub struct Room {
    // Static data
    pub name: String,
    pub password: Option<String>, // If None, room is public
    // explicit_songs: bool,
    pub num_rounds: u32,
    pub round_time_secs: u32,
    pub created_at: std::time::Instant,

    // Dynamic data, always present
    pub song_provider: std::sync::Arc<SongProvider>,
    pub players: Vec<Player>,
    pub state: RoomState,
    pub empty_last_time_we_checked: bool,

    // Dynamic data, only while playing
    pub current_round: u32, // zero-indexed
    pub round_task: Option<AttachedTask>,
    pub current_song: Option<Song>,
    pub round_start_time: Option<std::time::Instant>,
}

impl Room {
    pub fn send_all(&self, msg: &SendEvent) {
        for player in &self.players {
            player.send(msg);
        }
    }

    pub fn player_state_msg(&self) -> SendEvent {
        SendEvent::PlayerData {
            payload: self.players.iter().map(|p| p.to_player_data()).collect(),
            owner: self.players.first().unwrap().id,
        }
    }
}
