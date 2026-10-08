use axum::body::Bytes;
use axum::extract::ws::{Message, WebSocket, WebSocketUpgrade};
use axum::extract::State;
use axum::response::IntoResponse;
use axum::routing::get;
use axum::Router;
use tokio::sync::watch;
use tokio_util::sync::CancellationToken;
use tower_http::trace::TraceLayer;

#[derive(Clone)]
pub struct AppState {
    pub welcome: Bytes,
    pub frames: watch::Receiver<Bytes>,
    pub shutdown: CancellationToken,
}

pub fn router(state: AppState) -> Router {
    Router::new()
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
        shutdown,
    } = state;

    if socket.send(Message::Binary(welcome)).await.is_err() {
        return;
    }

    loop {
        tokio::select! {
            _ = shutdown.cancelled() => break,
            changed = frames.changed() => {
                if changed.is_err() {
                    break;
                }
                let frame = frames.borrow_and_update().clone();
                if socket.send(Message::Binary(frame)).await.is_err() {
                    break;
                }
            }
            incoming = socket.recv() => match incoming {
                None | Some(Err(_)) | Some(Ok(Message::Close(_))) => break,
                Some(Ok(_)) => {}
            },
        }
    }
}
