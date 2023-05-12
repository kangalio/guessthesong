#[derive(Debug)]
pub struct WebSocket {
    send: std::sync::Arc<
        tokio::sync::Mutex<
            futures::stream::SplitSink<
                // tokio_tungstenite::WebSocketStream<tokio::net::TcpStream>,
                // tungstenite::Message,
                axum::extract::ws::WebSocket,
                axum::extract::ws::Message,
            >,
        >,
    >,
    recv: tokio::sync::Mutex<
        futures::stream::SplitStream<
            // tokio_tungstenite::WebSocketStream<tokio::net::TcpStream>
            axum::extract::ws::WebSocket,
        >,
    >,
    // Need to store this for send-only users, who would otherwise would never see notice the closed
    // stream
    is_closed: std::sync::Arc<parking_lot::Mutex<bool>>,
}

impl WebSocket {
    pub fn new(ws: axum::extract::ws::WebSocket) -> Self {
        use futures::StreamExt as _;

        let (send, recv) = ws.split();
        Self {
            send: std::sync::Arc::new(tokio::sync::Mutex::new(send)),
            recv: tokio::sync::Mutex::new(recv),
            is_closed: std::sync::Arc::new(parking_lot::Mutex::new(false)),
        }
    }

    /// Returns None when stream is closed
    pub fn send(&self, msg: &impl serde::Serialize) -> Option<()> {
        use axum::extract::ws::Message;
        use futures::SinkExt as _;

        if *self.is_closed.lock() {
            return None;
        }

        let msg = match serde_json::to_string(msg) {
            Ok(x) => Message::Text(x),
            Err(e) => {
                log::warn!("failed to serialize websocket message: {}", e);
                return Some(());
            }
        };

        let send = self.send.clone();
        let is_closed = self.is_closed.clone();
        tokio::spawn(async move {
            if let Err(_) = send.lock().await.send(msg).await {
                *is_closed.lock() = true;
            }
        });

        Some(())
    }

    /// Returns None when stream is closed
    pub async fn recv<T: serde::de::DeserializeOwned>(&self) -> Option<T> {
        use axum::extract::ws::Message;
        use futures::StreamExt as _;

        loop {
            match self.recv.lock().await.next().await {
                Some(Ok(Message::Text(text))) => match serde_json::from_str(&text) {
                    Ok(x) => return Some(x),
                    Err(e) => log::warn!("failed to deserialize websocket message: {}", e),
                },
                // Mmh yes let's have 134513 states for the same thing
                None | Some(Err(_)) | Some(Ok(Message::Close(_))) => return None,
                Some(Ok(other)) => log::warn!("ignoring unexpected websocket message: {:?}", other),
            }
        }
    }
}

/// See [`spawn_attached`]
#[must_use = "dropping this type aborts the task"]
pub struct AttachedTask(tokio::task::JoinHandle<()>);
impl Drop for AttachedTask {
    fn drop(&mut self) {
        self.0.abort();
    }
}
/// Wrapper around [`tokio::spawn`] that aborts the task instead of detaching when dropped
///
/// Useful for utility tasks that shouldn't outlive their "parent" task
pub fn spawn_attached(f: impl std::future::Future<Output = ()> + Send + 'static) -> AttachedTask {
    AttachedTask(tokio::spawn(f))
}

/// FUCK YOU futures for making every interaction with you an absolute pain in the ass
///
/// This wrapper function is how a functioning brain would do API design
pub async fn select_all<T>(
    futures: impl IntoIterator<Item = impl std::future::Future<Output = T>>,
) -> T {
    // use std::future::Future as _;
    // let mut futures = futures.into_iter().map(Box::pin).collect::<Vec<_>>();
    // std::future::poll_fn(move |cx| {
    //     for mut future in std::mem::take(&mut futures) {
    //         match std::pin::Pin::new(&mut future).poll(cx) {
    //             std::task::Poll::Ready(value) => return std::task::Poll::Ready(value),
    //             std::task::Poll::Pending => {}
    //         }
    //     }
    //     std::task::Poll::Pending
    // })
    // .await

    // use futures::StreamExt as _;
    // match futures.into_iter().collect::<futures::stream::FuturesUnordered<_>>().next().await {
    //     Some(x) => x,
    //     None => std::future::pending().await,
    // }

    let futures = futures.into_iter().collect::<Vec<_>>();
    // Guard against select_all's braindead panic
    if futures.is_empty() {
        return std::future::pending().await;
    }
    futures::future::select_all(futures.into_iter().map(Box::pin)).await.0
}
