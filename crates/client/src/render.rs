//! Draws the latest [`State`] into the window.

use std::collections::HashMap;

use eframe::egui::{self, Align2, Color32, FontId, Pos2, Rect, Stroke, Vec2};
use sim::{EntityId, Species};

use crate::State;

const BACKGROUND: Color32 = Color32::from_rgb(10, 14, 35);
const NIGHT_SKY: [f32; 3] = [10.0, 14.0, 35.0];
const DAY_GRASS: [f32; 3] = [104.0, 158.0, 86.0];
const PERSON: Color32 = Color32::from_rgb(255, 209, 102);

pub fn draw(ui: &mut egui::Ui, state: &State) {
    let screen = ui.max_rect();
    let painter = ui.painter();
    painter.rect_filled(screen, 0.0, BACKGROUND);

    if let Some(view) = &state.view {
        let (world_w, world_h) = state.world_size;
        let scale = (screen.width() / world_w).min(screen.height() / world_h);
        let world = Rect::from_center_size(screen.center(), Vec2::new(world_w, world_h) * scale);
        let to_screen = |x: f32, y: f32| world.min + Vec2::new(x, y) * scale;

        painter.rect_filled(world, 0.0, ground_colour(view.daylight));

        let positions: HashMap<EntityId, Pos2> = view
            .people
            .iter()
            .map(|p| (p.id, to_screen(p.x, p.y)))
            .collect();
        for bond in &view.bonds {
            if let (Some(&a), Some(&b)) = (positions.get(&bond.a), positions.get(&bond.b)) {
                let alpha = (bond.affinity.clamp(0.0, 1.0) * 255.0) as u8;
                let colour = Color32::from_rgba_unmultiplied(255, 120, 160, alpha);
                painter.line_segment([a, b], Stroke::new(1.5, colour));
            }
        }

        for animal in &view.animals {
            let (colour, size) = match animal.species {
                Species::Deer => (Color32::from_rgb(168, 116, 58), 5.0),
                Species::Rabbit => (Color32::from_rgb(230, 224, 212), 3.0),
                Species::Wolf => (Color32::from_rgb(107, 111, 122), 5.0),
            };
            let centre = to_screen(animal.x, animal.y);
            painter.rect_filled(
                Rect::from_center_size(centre, Vec2::splat(size)),
                0.0,
                colour,
            );
        }

        let name_font = FontId::proportional(11.0);
        let name_colour = Color32::from_white_alpha(217);
        for person in &view.people {
            let pos = positions[&person.id];
            painter.circle_filled(pos, 4.0, PERSON);
            painter.text(
                pos + Vec2::new(6.0, -6.0),
                Align2::LEFT_BOTTOM,
                &person.name,
                name_font.clone(),
                name_colour,
            );
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
        draw_label(
            painter,
            &hud,
            screen.left_top() + Vec2::new(12.0, 8.0),
            Align2::LEFT_TOP,
        );
    }

    if let Some(status) = &state.status {
        draw_label(
            painter,
            status,
            screen.left_bottom() + Vec2::new(12.0, -10.0),
            Align2::LEFT_BOTTOM,
        );
    }
}

/// White text with a drop shadow, readable on both day and night ground.
fn draw_label(painter: &egui::Painter, text: &str, pos: Pos2, anchor: Align2) {
    let font = FontId::proportional(14.0);
    let shadow = Color32::from_black_alpha(140);
    painter.text(pos + Vec2::splat(1.0), anchor, text, font.clone(), shadow);
    painter.text(pos, anchor, text, font, Color32::WHITE);
}

fn ground_colour(daylight: f32) -> Color32 {
    let mix = |i: usize| (NIGHT_SKY[i] + (DAY_GRASS[i] - NIGHT_SKY[i]) * daylight).round() as u8;
    Color32::from_rgb(mix(0), mix(1), mix(2))
}
