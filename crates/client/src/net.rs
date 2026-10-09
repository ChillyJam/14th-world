//! Background thread that keeps a WebSocket open to the server and copies
//! every message into the shared [`State`].

use std::sync::PoisonError;
use std::thread;
use std::time::Duration;

use eframe::egui;
use protocol::{ServerMsg, PROTOCOL_VERSION};
use tungstenite::Message;

use crate::{Shared, State};

const RECONNECT_DELAY: Duration = Duration::from_secs(2);

/// Connects to `url` and reconnects forever. The thread ends with the process.
pub fn spawn(url: String, state: Shared, ctx: egui::Context) {
    thread::Builder::new()
        .name("net".into())
        .spawn(move || loop {
            let reason = match stream(&url, &state, &ctx) {
                Ok(()) => "Disconnected".to_owned(),
                Err(err) => format!("Disconnected ({err})"),
            };
            lock(&state).status = Some(format!("{reason}, reconnecting…"));
            ctx.request_repaint();
            thread::sleep(RECONNECT_DELAY);
        })
        .expect("spawning network thread");
}

/// Reads messages until the connection closes.
fn stream(url: &str, state: &Shared, ctx: &egui::Context) -> tungstenite::Result<()> {
    let (mut socket, _) = tungstenite::connect(url)?;
    loop {
        match socket.read()? {
            Message::Binary(bytes) => match protocol::decode(&bytes) {
                Ok(msg) => {
                    apply(&mut lock(state), msg);
                    ctx.request_repaint();
                }
                Err(err) => eprintln!("bad message: {err}"),
            },
            Message::Close(_) => return Ok(()),
            _ => {}
        }
    }
}

fn apply(state: &mut State, msg: ServerMsg) {
    match msg {
        ServerMsg::Welcome {
            protocol_version,
            world_width,
            world_height,
        } => {
            state.world_size = (world_width, world_height);
            state.status = (protocol_version != PROTOCOL_VERSION)
                .then(|| "The server runs a different version, please update the client.".into());
        }
        ServerMsg::Frame(view) => state.view = Some(view),
    }
}

/// The UI keeps drawing whatever is there, so a poisoned lock is still usable.
pub fn lock(state: &Shared) -> std::sync::MutexGuard<'_, State> {
    state.lock().unwrap_or_else(PoisonError::into_inner)
}
