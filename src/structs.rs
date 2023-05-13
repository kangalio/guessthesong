use crate::song_provider::*;
use crate::utils::*;

#[derive(PartialEq, Eq, Copy, Clone, Debug)]
pub struct PlayerId(pub u32);
impl serde::Serialize for PlayerId {
    fn serialize<S: serde::Serializer>(&self, serializer: S) -> Result<S::Ok, S::Error> {
        self.0.to_string().serialize(serializer)
    }
}

#[derive(serde::Deserialize, Debug)]
#[serde(tag = "type")]
#[serde(rename_all = "kebab-case")]
pub enum ReceiveEvent {
    IncomingMsg { msg: String },
    StartGame,
    AudioLoaded,
    TypingStatus { typing: bool },
}

#[derive(serde::Serialize)]
pub struct SinglePlayerData {
    pub uuid: PlayerId,
    pub username: String,
    pub points: u32,
    pub prev_points: u32,
    pub streak: u32,
    pub emoji: String,
    pub loaded: bool,
    pub guessed: bool,
    pub disconnected: bool,
}

#[derive(serde::Serialize)]
pub struct ScoreboardPlayer {
    pub uuid: PlayerId,
    pub display_name: String,
    pub points: u32,
    pub points_diff: u32,
    pub prev_points: u32,
    pub streak: u32,
}

#[derive(serde::Serialize)]
pub enum ListedRoomState {
    Private,
    Public,
}

#[derive(serde::Serialize)]
pub struct ListedRoom {
    pub code: u32,
    pub game_mode: String,
    pub idle: u64,
    pub name: String,
    pub players: usize,
    pub status: ListedRoomState,
    pub theme: String,
}

#[derive(serde::Serialize)]
#[serde(tag = "state")]
#[serde(rename_all = "snake_case")]
pub enum SendEvent {
    FetchNew { msg: Vec<ListedRoom> },
    Join { message: String, payload: Box<SendEvent> },
    PlayerData { payload: Vec<SinglePlayerData>, owner: PlayerId },
    Chat { r#type: String, username: String, uuid: PlayerId, msg: String },
    Loading,
    Timer { message: u32, hint: String, scores: Vec<SinglePlayerData>, round_time: u32 },
    PlayerTyping { uuid: PlayerId, typing: bool },
    Notify { message: String },
    NewTurn,
    Scoreboard { payload: Vec<ScoreboardPlayer>, round: u32, max_rounds: u32 },
    StartGame,
}

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
}

impl Player {
    pub fn to_scoreboard_player(&self) -> ScoreboardPlayer {
        ScoreboardPlayer {
            uuid: self.id,
            display_name: self.name.clone(),
            points: self.points,
            points_diff: self.guessed.unwrap_or(0),
            prev_points: self.points - self.guessed.unwrap_or(0),
            streak: self.streak,
        }
    }

    pub fn to_player_data(&self) -> SinglePlayerData {
        SinglePlayerData {
            uuid: self.id,
            username: self.name.clone(),
            points: self.points,
            prev_points: self.points - self.guessed.unwrap_or(0),
            streak: self.streak,
            emoji: self.emoji.clone(),
            loaded: self.loaded,
            guessed: self.guessed.is_some(),
            disconnected: self.ws.is_none(),
        }
    }
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

    // Dynamic data, always present
    pub song_provider: std::sync::Arc<SongProvider>,
    pub players: Vec<Player>,
    pub state: RoomState,

    // Dynamic data, only while playing
    pub current_round: u32, // zero-indexed
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

    pub fn player_state_msg(&self) -> SendEvent {
        SendEvent::PlayerData {
            payload: self.players.iter().map(|p| p.to_player_data()).collect(),
            owner: self.players.first().unwrap().id,
        }
    }
}
