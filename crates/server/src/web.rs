use std::sync::{Arc, Mutex};

use axum::extract::ws::{Message, WebSocket, WebSocketUpgrade};
use axum::extract::State;
use axum::response::{Html, IntoResponse};
use axum::routing::get;
use axum::Router;
use protocol::{EventLog, ServerMsg};
use tokio::sync::watch;
use tokio_util::sync::CancellationToken;
use tower_http::trace::TraceLayer;

#[derive(Clone)]
pub struct AppState {
    pub welcome: String,
    pub frames: watch::Receiver<String>,
    pub log: Arc<Mutex<EventLog>>,
    pub shutdown: CancellationToken,
}

/// The browser client, compiled into the binary so the server is one file.
const INDEX_HTML: &str = include_str!("../assets/index.html");

pub fn router(state: AppState) -> Router {
    Router::new()
        .route("/", get(|| async { Html(INDEX_HTML) }))
        .route("/ws", get(ws_upgrade))
        .route("/healthz", get(|| async { "ok" }))
        .layer(TraceLayer::new_for_http())
        .with_state(state)
}

async fn ws_upgrade(ws: WebSocketUpgrade, State(state): State<AppState>) -> impl IntoResponse {
    ws.on_upgrade(move |socket| stream_frames(socket, state))
}

/// Pushes the latest frame whenever the world changes. A slow client simply
/// skips frames instead of building up a backlog.
async fn stream_frames(mut socket: WebSocket, state: AppState) {
    let AppState {
        welcome,
        mut frames,
        log,
        shutdown,
    } = state;

    if socket.send(Message::Text(welcome.into())).await.is_err() {
        return;
    }

    // The first Log message carries the whole remembered history.
    let mut sent_seq = 0;

    loop {
        tokio::select! {
            _ = shutdown.cancelled() => break,
            changed = frames.changed() => {
                if changed.is_err() {
                    break;
                }
                let frame = frames.borrow_and_update().clone();
                let (entries, next_seq) = {
                    let log = log.lock().expect("log lock poisoned");
                    (log.since(sent_seq), log.next_seq())
                };
                if socket.send(Message::Text(frame.into())).await.is_err() {
                    break;
                }
                if !entries.is_empty() {
                    sent_seq = next_seq;
                    let msg = protocol::encode(&ServerMsg::Log(entries));
                    if socket.send(Message::Text(msg.into())).await.is_err() {
                        break;
                    }
                }
            }
            incoming = socket.recv() => match incoming {
                None | Some(Err(_)) | Some(Ok(Message::Close(_))) => break,
                Some(Ok(_)) => {}
            },
        }
    }
}
