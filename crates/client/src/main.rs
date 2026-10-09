//! Native desktop client. Connects to a world server's `/ws` endpoint,
//! receives [`WorldView`] frames on a background thread and draws them with
//! egui.
//!
//! Usage: `world-client [SERVER]`, where `SERVER` is either a full WebSocket
//! URL (`wss://example.com/ws`) or just `host:port`. Falls back to the
//! `WORLD_SERVER` environment variable, then to a server on this machine.

// Release builds are GUI apps on Windows, so don't open a console window.
#![cfg_attr(not(debug_assertions), windows_subsystem = "windows")]

mod net;
mod render;

use std::sync::{Arc, Mutex};

use eframe::egui;
use protocol::WorldView;

const DEFAULT_SERVER: &str = "127.0.0.1:8080";

/// Everything the network thread has received, read by the UI every frame.
pub struct State {
    world_size: (f32, f32),
    view: Option<WorldView>,
    status: Option<String>,
}

pub type Shared = Arc<Mutex<State>>;

struct App {
    state: Shared,
}

impl eframe::App for App {
    fn ui(&mut self, ui: &mut egui::Ui, _frame: &mut eframe::Frame) {
        render::draw(ui, &net::lock(&self.state));
    }
}

fn main() -> eframe::Result {
    let server = std::env::args()
        .nth(1)
        .or_else(|| std::env::var("WORLD_SERVER").ok())
        .unwrap_or_else(|| DEFAULT_SERVER.into());
    let url = server_url(&server);

    let state: Shared = Arc::new(Mutex::new(State {
        world_size: (1.0, 1.0),
        view: None,
        status: Some(format!("Connecting to {url}…")),
    }));

    let options = eframe::NativeOptions {
        viewport: egui::ViewportBuilder::default()
            .with_title("14th World")
            .with_inner_size([1280.0, 800.0])
            .with_min_inner_size([320.0, 240.0]),
        ..Default::default()
    };
    eframe::run_native(
        "14th World",
        options,
        Box::new(move |cc| {
            net::spawn(url, state.clone(), cc.egui_ctx.clone());
            Ok(Box::new(App { state }))
        }),
    )
}

/// Accepts either a full `ws://` / `wss://` URL or a bare `host:port`.
fn server_url(server: &str) -> String {
    if server.contains("://") {
        server.to_owned()
    } else {
        format!("ws://{}/ws", server.trim_end_matches('/'))
    }
}

#[cfg(test)]
mod tests {
    use super::server_url;

    #[test]
    fn bare_host_gets_scheme_and_path() {
        assert_eq!(server_url("127.0.0.1:8080"), "ws://127.0.0.1:8080/ws");
        assert_eq!(server_url("example.com/"), "ws://example.com/ws");
    }

    #[test]
    fn full_url_is_kept() {
        assert_eq!(server_url("wss://example.com/ws"), "wss://example.com/ws");
    }
}
