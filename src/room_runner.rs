use crate::*;

#[derive(Debug)]
pub enum RoomRunnerMessage {
    PlayerJoin(Player),
    WebsocketConnect { player_id: PlayerId, ws: axum::extract::ws::WebSocket },
}

#[derive(Debug)]
pub struct Player {
    pub ws: Option<WebSocket>,
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
enum RoomState {
    Lobby,
    Play,
}

#[derive(Clone, serde::Deserialize)]
struct Song {
    // path: std::path::PathBuf,
    artist: String,
    title: String,
    #[serde(skip)]
    audio: Option<std::sync::Arc<Vec<u8>>>,
}

struct Room {
    meta: std::sync::Arc<crate::RoomMeta>,
    players: Vec<Player>,
}

async fn ws_send_to_all(room: &Room, msg: &impl serde::Serialize) {
    for player in &room.players {
        if let Some(ws) = &player.ws {
            ws.send(msg).await.unwrap();
        }
    }
}

fn player_state_msg(room: &Room, joined: Option<&str>) -> SendEvent {
    SendEvent::PlayerData {
        payload: room
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
        owner: room.players.first().unwrap().id,
    }
}

async fn ws_recv(room: &Room) -> (usize, ReceiveEvent) {
    select_all(room.players.iter().enumerate().filter_map(|(i, p)| {
        let ws = p.ws.as_ref()?;
        Some(Box::pin(async move { (i, ws.recv().await.unwrap()) }))
    }))
    .await
}

pub async fn room_runner(
    room_meta: std::sync::Arc<crate::RoomMeta>,
    mut events: tokio::sync::mpsc::UnboundedReceiver<RoomRunnerMessage>,
) {
    let mut room = Room { meta: room_meta, players: Vec::new() };

    loop {
        tokio::select! {
            (i, event) = ws_recv(&room) => match event {
                ReceiveEvent::IncomingMsg { msg } => {
                    let player = &room.players[i];
                    let msg = SendEvent::Chat {
                        r#type: "message".into(),
                        uuid: player.id,
                        username: player.name.clone(),
                        msg: msg,
                    };
                    ws_send_to_all(&mut room, &msg).await;
                }
                _ => {},
            },
            event = events.recv() => match event.unwrap() {
                RoomRunnerMessage::PlayerJoin(player) => {
                    room.players.push(player);
                    room.meta.player_ids.lock().push(player.id);
                    let player = room.players.last().unwrap();
                    let msg = SendEvent::Join { message: player.name.clone(), payload: Box::new(player_state_msg(&room, Some(&player.name))) };

                    ws_send_to_all(&mut room, &msg).await;
                }
                RoomRunnerMessage::WebsocketConnect { player_id, ws } => {
                    let ws = WebSocket::new(ws);
                    for player in &room.players {
                        let msg = SendEvent::Join { message: player.name.clone(), payload: Box::new(player_state_msg(&room, Some(&player.name))) };
                        ws.send(&msg).await;
                    }
                    if let Some(player) = room.players.iter_mut().find(|p| p.id == player_id) {
                        player.ws = Some(ws);
                    }
                }
            },
        }
    }
}
