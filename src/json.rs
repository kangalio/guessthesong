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
