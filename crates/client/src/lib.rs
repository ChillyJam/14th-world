//! Browser client. Connects to the server's `/ws` endpoint, receives
//! [`WorldView`] frames and draws them on the `#world` canvas.

use std::cell::RefCell;
use std::collections::HashMap;
use std::rc::Rc;

use protocol::{ServerMsg, WorldView, PROTOCOL_VERSION};
use sim::{EntityId, Species};
use wasm_bindgen::prelude::*;
use wasm_bindgen::JsCast;
use web_sys::{
    BinaryType, CanvasRenderingContext2d, HtmlCanvasElement, MessageEvent, WebSocket, Window,
};

const RECONNECT_DELAY_MS: i32 = 2_000;
const NIGHT_SKY: [f32; 3] = [10.0, 14.0, 35.0];
const DAY_GRASS: [f32; 3] = [104.0, 158.0, 86.0];

struct State {
    world_size: (f32, f32),
    view: Option<WorldView>,
    status: Option<String>,
}

type Shared = Rc<RefCell<State>>;
type FrameCallback = Rc<RefCell<Option<Closure<dyn FnMut()>>>>;

#[wasm_bindgen(start)]
pub fn start() -> Result<(), JsValue> {
    console_error_panic_hook::set_once();

    let window = web_sys::window().ok_or("no window")?;
    let canvas: HtmlCanvasElement = window
        .document()
        .ok_or("no document")?
        .get_element_by_id("world")
        .ok_or("missing #world canvas")?
        .dyn_into()?;
    let ctx: CanvasRenderingContext2d = canvas
        .get_context("2d")?
        .ok_or("2d canvas unavailable")?
        .dyn_into()?;

    let state: Shared = Rc::new(RefCell::new(State {
        world_size: (1.0, 1.0),
        view: None,
        status: Some("Connecting…".into()),
    }));

    connect(&window, state.clone())?;
    render_loop(window, canvas, ctx, state);
    Ok(())
}

fn connect(window: &Window, state: Shared) -> Result<(), JsValue> {
    let location = window.location();
    let scheme = if location.protocol()? == "https:" {
        "wss"
    } else {
        "ws"
    };
    let ws = WebSocket::new(&format!("{scheme}://{}/ws", location.host()?))?;
    ws.set_binary_type(BinaryType::Arraybuffer);

    let on_message = {
        let state = state.clone();
        Closure::<dyn FnMut(MessageEvent)>::new(move |event: MessageEvent| {
            let Ok(buffer) = event.data().dyn_into::<js_sys::ArrayBuffer>() else {
                return;
            };
            let bytes = js_sys::Uint8Array::new(&buffer).to_vec();
            let mut state = state.borrow_mut();
            match protocol::decode(&bytes) {
                Ok(ServerMsg::Welcome {
                    protocol_version,
                    world_width,
                    world_height,
                }) => {
                    state.world_size = (world_width, world_height);
                    state.status = (protocol_version != PROTOCOL_VERSION)
                        .then(|| "Server was updated, please reload the page.".into());
                }
                Ok(ServerMsg::Frame(view)) => state.view = Some(view),
                Err(err) => web_sys::console::warn_1(&format!("bad message: {err}").into()),
            }
        })
    };
    ws.set_onmessage(Some(on_message.as_ref().unchecked_ref()));
    on_message.forget();

    let on_close = {
        let window = window.clone();
        Closure::<dyn FnMut()>::new(move || {
            state.borrow_mut().status = Some("Disconnected, reconnecting…".into());
            let retry = {
                let (window, state) = (window.clone(), state.clone());
                Closure::once_into_js(move || {
                    if let Err(err) = connect(&window, state) {
                        web_sys::console::error_1(&err);
                    }
                })
            };
            let _ = window.set_timeout_with_callback_and_timeout_and_arguments_0(
                retry.unchecked_ref(),
                RECONNECT_DELAY_MS,
            );
        })
    };
    ws.set_onclose(Some(on_close.as_ref().unchecked_ref()));
    on_close.forget();

    Ok(())
}

fn render_loop(
    window: Window,
    canvas: HtmlCanvasElement,
    ctx: CanvasRenderingContext2d,
    state: Shared,
) {
    // The callback must be able to schedule itself, hence the shared slot.
    let slot: FrameCallback = Rc::new(RefCell::new(None));
    let schedule = slot.clone();
    let win = window.clone();
    *slot.borrow_mut() = Some(Closure::new(move || {
        draw(&win, &canvas, &ctx, &state.borrow());
        if let Some(callback) = schedule.borrow().as_ref() {
            let _ = win.request_animation_frame(callback.as_ref().unchecked_ref());
        }
    }));
    if let Some(callback) = slot.borrow().as_ref() {
        let _ = window.request_animation_frame(callback.as_ref().unchecked_ref());
    };
}

fn draw(
    window: &Window,
    canvas: &HtmlCanvasElement,
    ctx: &CanvasRenderingContext2d,
    state: &State,
) {
    // Match the backing store to the displayed size for crisp rendering.
    let dpr = window.device_pixel_ratio();
    let width = (canvas.client_width() as f64 * dpr) as u32;
    let height = (canvas.client_height() as f64 * dpr) as u32;
    if canvas.width() != width || canvas.height() != height {
        canvas.set_width(width);
        canvas.set_height(height);
    }
    let (w, h) = (width as f64, height as f64);

    ctx.set_fill_style_str("#0a0e23");
    ctx.fill_rect(0.0, 0.0, w, h);

    if let Some(view) = &state.view {
        let (world_w, world_h) = state.world_size;
        let scale = (w / world_w as f64).min(h / world_h as f64);
        let origin = (
            (w - world_w as f64 * scale) / 2.0,
            (h - world_h as f64 * scale) / 2.0,
        );
        let to_screen = |x: f32, y: f32| (origin.0 + x as f64 * scale, origin.1 + y as f64 * scale);

        ctx.set_fill_style_str(&ground_colour(view.daylight));
        ctx.fill_rect(
            origin.0,
            origin.1,
            world_w as f64 * scale,
            world_h as f64 * scale,
        );

        let positions: HashMap<EntityId, (f32, f32)> =
            view.people.iter().map(|p| (p.id, (p.x, p.y))).collect();
        ctx.set_line_width(1.5 * dpr);
        for bond in &view.bonds {
            if let (Some(&a), Some(&b)) = (positions.get(&bond.a), positions.get(&bond.b)) {
                let (ax, ay) = to_screen(a.0, a.1);
                let (bx, by) = to_screen(b.0, b.1);
                ctx.set_stroke_style_str(&format!("rgba(255, 120, 160, {})", bond.affinity));
                ctx.begin_path();
                ctx.move_to(ax, ay);
                ctx.line_to(bx, by);
                ctx.stroke();
            }
        }

        for animal in &view.animals {
            let (x, y) = to_screen(animal.x, animal.y);
            let (colour, size) = match animal.species {
                Species::Deer => ("#a8743a", 5.0),
                Species::Rabbit => ("#e6e0d4", 3.0),
                Species::Wolf => ("#6b6f7a", 5.0),
            };
            let size = size * dpr;
            ctx.set_fill_style_str(colour);
            ctx.fill_rect(x - size / 2.0, y - size / 2.0, size, size);
        }

        ctx.set_font(&format!("{}px system-ui, sans-serif", (11.0 * dpr).round()));
        for person in &view.people {
            let (x, y) = to_screen(person.x, person.y);
            ctx.set_fill_style_str("#ffd166");
            ctx.begin_path();
            let _ = ctx.arc(x, y, 4.0 * dpr, 0.0, std::f64::consts::TAU);
            ctx.fill();
            ctx.set_fill_style_str("rgba(255, 255, 255, 0.85)");
            let _ = ctx.fill_text(&person.name, x + 6.0 * dpr, y - 6.0 * dpr);
        }

        let hud = format!(
            "Year {}, day {} · {:02}:{:02} · {} · population {} · knowledge {:.0}",
            view.year + 1,
            view.day_of_year + 1,
            view.minute_of_day / 60,
            view.minute_of_day % 60,
            view.era.name(),
            view.people.len(),
            view.knowledge,
        );
        draw_label(ctx, &hud, 12.0 * dpr, 22.0 * dpr, dpr);
    }

    if let Some(status) = &state.status {
        draw_label(ctx, status, 12.0 * dpr, h - 14.0 * dpr, dpr);
    }
}

fn draw_label(ctx: &CanvasRenderingContext2d, text: &str, x: f64, y: f64, dpr: f64) {
    ctx.set_font(&format!("{}px system-ui, sans-serif", (14.0 * dpr).round()));
    ctx.set_fill_style_str("rgba(0, 0, 0, 0.55)");
    let _ = ctx.fill_text(text, x + dpr, y + dpr);
    ctx.set_fill_style_str("#ffffff");
    let _ = ctx.fill_text(text, x, y);
}

fn ground_colour(daylight: f32) -> String {
    let mix = |i: usize| NIGHT_SKY[i] + (DAY_GRASS[i] - NIGHT_SKY[i]) * daylight;
    format!("rgb({:.0}, {:.0}, {:.0})", mix(0), mix(1), mix(2))
}
