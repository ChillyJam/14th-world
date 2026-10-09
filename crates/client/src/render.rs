//! Draws the latest [`State`] into the window, and lets the user click a
//! person, animal or material deposit to see its stats.

use std::collections::HashMap;

use eframe::egui::{
    self, Align2, Color32, CursorIcon, FontId, Key, Pos2, Rect, Sense, Stroke, Vec2,
};
use protocol::{AnimalView, DepositView, PersonView, WorldView};
use sim::{EntityId, Era, Material, Species, WorldTime, DAYS_PER_YEAR, TICKS_PER_DAY};

use crate::State;

const BACKGROUND: Color32 = Color32::from_rgb(10, 14, 35);
const NIGHT_SKY: [f32; 3] = [10.0, 14.0, 35.0];
const DAY_GRASS: [f32; 3] = [104.0, 158.0, 86.0];
const PERSON: Color32 = Color32::from_rgb(255, 209, 102);
const SELECTION: Color32 = Color32::WHITE;
/// How close, in screen pixels, a click has to land to pick something.
const PICK_RADIUS: f32 = 10.0;

pub fn draw(ui: &mut egui::Ui, state: &State, selected: &mut Option<EntityId>) {
    let screen = ui.max_rect();
    let painter = ui.painter();
    painter.rect_filled(screen, 0.0, BACKGROUND);

    if let Some(view) = &state.view {
        let (world_w, world_h) = state.world_size;
        let scale = (screen.width() / world_w).min(screen.height() / world_h);
        let world = Rect::from_center_size(screen.center(), Vec2::new(world_w, world_h) * scale);
        let to_screen = |x: f32, y: f32| world.min + Vec2::new(x, y) * scale;

        painter.rect_filled(world, 0.0, ground_colour(view.daylight));

        for deposit in &view.deposits {
            draw_deposit(painter, deposit, to_screen(deposit.x, deposit.y));
        }

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
            let (colour, size) = animal_style(animal.species);
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

        let response = ui.interact(screen, ui.id().with("world"), Sense::click());
        let hovered = response
            .hover_pos()
            .and_then(|pointer| pick(view, to_screen, pointer));
        if hovered.is_some() {
            ui.ctx().set_cursor_icon(CursorIcon::PointingHand);
        }
        if response.clicked() {
            *selected = hovered;
        }
        if ui.input(|i| i.key_pressed(Key::Escape)) {
            *selected = None;
        }

        if let Some(id) = *selected {
            match Selected::find(view, id) {
                Some(entity) => {
                    let (x, y) = entity.position();
                    painter.circle_stroke(to_screen(x, y), 8.0, Stroke::new(1.5, SELECTION));
                    let mut open = true;
                    egui::Window::new(entity.title())
                        .id(ui.id().with("stats"))
                        .open(&mut open)
                        .anchor(Align2::RIGHT_TOP, Vec2::new(-12.0, 36.0))
                        .collapsible(false)
                        .resizable(false)
                        .show(ui.ctx(), |ui| match entity {
                            Selected::Person(person) => person_stats(ui, view, person),
                            Selected::Animal(animal) => animal_stats(ui, animal),
                            Selected::Deposit(deposit) => deposit_stats(ui, view, deposit),
                        });
                    if !open {
                        *selected = None;
                    }
                }
                // It's gone from the world.
                None => *selected = None,
            }
        }
    }

    if let Some(status) = &state.status {
        draw_label(
            ui.painter(),
            status,
            screen.left_bottom() + Vec2::new(12.0, -10.0),
            Align2::LEFT_BOTTOM,
        );
    }
}

#[derive(Clone, Copy)]
enum Selected<'a> {
    Person(&'a PersonView),
    Animal(&'a AnimalView),
    Deposit(&'a DepositView),
}

impl<'a> Selected<'a> {
    fn find(view: &'a WorldView, id: EntityId) -> Option<Self> {
        let person = view.people.iter().find(|p| p.id == id).map(Self::Person);
        person
            .or_else(|| view.animals.iter().find(|a| a.id == id).map(Self::Animal))
            .or_else(|| view.deposits.iter().find(|d| d.id == id).map(Self::Deposit))
    }

    fn position(self) -> (f32, f32) {
        match self {
            Self::Person(p) => (p.x, p.y),
            Self::Animal(a) => (a.x, a.y),
            Self::Deposit(d) => (d.x, d.y),
        }
    }

    fn title(self) -> String {
        match self {
            Self::Person(p) => p.name.clone(),
            Self::Animal(a) => format!("{} #{}", a.species.name(), a.id),
            Self::Deposit(d) => format!("{} #{}", d.material.deposit_name(), d.id),
        }
    }
}

/// The person or animal closest to `pointer`, if any is within [`PICK_RADIUS`].
/// Otherwise the closest deposit, so a person standing by a tree stays easy
/// to click.
fn pick(view: &WorldView, to_screen: impl Fn(f32, f32) -> Pos2, pointer: Pos2) -> Option<EntityId> {
    let nearest = |candidates: Vec<(EntityId, f32, f32)>| {
        candidates
            .into_iter()
            .map(|(id, x, y)| (id, to_screen(x, y).distance(pointer)))
            .filter(|&(_, dist)| dist <= PICK_RADIUS)
            .min_by(|a, b| a.1.total_cmp(&b.1))
            .map(|(id, _)| id)
    };
    let people = view.people.iter().map(|p| (p.id, p.x, p.y));
    let animals = view.animals.iter().map(|a| (a.id, a.x, a.y));
    nearest(people.chain(animals).collect())
        .or_else(|| nearest(view.deposits.iter().map(|d| (d.id, d.x, d.y)).collect()))
}

fn person_stats(ui: &mut egui::Ui, view: &WorldView, person: &PersonView) {
    let asleep = WorldTime { tick: view.tick }.is_night();
    let share = if view.knowledge > 0.0 {
        person.knowledge / view.knowledge * 100.0
    } else {
        0.0
    };
    egui::Grid::new("person").num_columns(2).show(ui, |ui| {
        stat(
            ui,
            "Age",
            format_duration(view.tick.saturating_sub(person.born_tick)),
        );
        stat(ui, "Born", format_date(person.born_tick));
        stat(ui, "Status", if asleep { "Asleep" } else { "Awake" });
        stat(
            ui,
            "Knowledge",
            format!("{:.1} ({share:.0}% of all)", person.knowledge),
        );
        stat(ui, "Has met", format!("{} people", person.acquaintances));
        stat(ui, "Carrying", format_inventory(&person.inventory));
        stat(ui, "Position", format!("{:.0}, {:.0}", person.x, person.y));
    });

    let names: HashMap<EntityId, &str> = view
        .people
        .iter()
        .map(|p| (p.id, p.name.as_str()))
        .collect();
    let mut bonds: Vec<_> = view
        .bonds
        .iter()
        .filter_map(|bond| {
            let other = match person.id {
                id if id == bond.a => bond.b,
                id if id == bond.b => bond.a,
                _ => return None,
            };
            Some((names.get(&other).copied().unwrap_or("?"), bond))
        })
        .collect();
    if bonds.is_empty() {
        return;
    }
    bonds.sort_by(|a, b| b.1.affinity.total_cmp(&a.1.affinity));

    ui.separator();
    ui.strong("Relationships");
    egui::Grid::new("bonds")
        .num_columns(3)
        .striped(true)
        .show(ui, |ui| {
            for (name, bond) in bonds {
                ui.label(name);
                ui.add(egui::ProgressBar::new(bond.affinity).desired_width(80.0))
                    .on_hover_text(format!("Affinity {:.2}", bond.affinity));
                ui.label(format!("met {}×", bond.encounters))
                    .on_hover_text(format!("First met {}", format_date(bond.first_met)));
                ui.end_row();
            }
        });
}

fn animal_stats(ui: &mut egui::Ui, animal: &AnimalView) {
    egui::Grid::new("animal").num_columns(2).show(ui, |ui| {
        stat(ui, "Species", animal.species.name());
        stat(
            ui,
            "Speed",
            format!("{:.1} per minute", animal.species.speed()),
        );
        stat(ui, "Position", format!("{:.0}, {:.0}", animal.x, animal.y));
    });
}

fn deposit_stats(ui: &mut egui::Ui, view: &WorldView, deposit: &DepositView) {
    let material = deposit.material;
    egui::Grid::new("deposit").num_columns(2).show(ui, |ui| {
        stat(ui, "Material", material.name());
        stat(
            ui,
            "Left",
            format!("{} of {}", deposit.amount, material.capacity()),
        );
        stat(
            ui,
            "Regrows",
            match material.regrowth_per_day() {
                0 => "Never".to_owned(),
                n => format!("{n} per day"),
            },
        );
        stat(ui, "Gatherable", gatherable(material, view.era));
        stat(
            ui,
            "Position",
            format!("{:.0}, {:.0}", deposit.x, deposit.y),
        );
    });
}

fn gatherable(material: Material, era: Era) -> String {
    if material.era() <= era {
        "Yes".to_owned()
    } else {
        format!("From the {}", material.era().name())
    }
}

/// E.g. "3 Wood, 1 Flint", or "Nothing".
fn format_inventory(inventory: &[(Material, u32)]) -> String {
    if inventory.is_empty() {
        return "Nothing".to_owned();
    }
    inventory
        .iter()
        .map(|(material, n)| format!("{n} {}", material.name()))
        .collect::<Vec<_>>()
        .join(", ")
}

fn stat(ui: &mut egui::Ui, label: &str, value: impl Into<egui::WidgetText>) {
    ui.weak(label);
    ui.label(value);
    ui.end_row();
}

fn animal_style(species: Species) -> (Color32, f32) {
    match species {
        Species::Deer => (Color32::from_rgb(168, 116, 58), 5.0),
        Species::Rabbit => (Color32::from_rgb(230, 224, 212), 3.0),
        Species::Wolf => (Color32::from_rgb(107, 111, 122), 5.0),
    }
}

/// Trees are round, everything dug out of the ground is square. Exhausted
/// deposits fade out until (if ever) they grow back.
fn draw_deposit(painter: &egui::Painter, deposit: &DepositView, centre: Pos2) {
    let (colour, size) = deposit_style(deposit.material);
    let colour = if deposit.amount == 0 {
        colour.gamma_multiply(0.3)
    } else {
        colour
    };
    if deposit.material == Material::Wood {
        painter.circle_filled(centre, size / 2.0, colour);
    } else {
        let square = Rect::from_center_size(centre, Vec2::splat(size));
        painter.rect_filled(square, 1.0, colour);
    }
}

fn deposit_style(material: Material) -> (Color32, f32) {
    match material {
        Material::Wood => (Color32::from_rgb(46, 94, 52), 9.0),
        Material::Stone => (Color32::from_rgb(150, 150, 140), 7.0),
        Material::Flint => (Color32::from_rgb(60, 64, 78), 5.0),
        Material::Clay => (Color32::from_rgb(178, 98, 64), 7.0),
        Material::Copper => (Color32::from_rgb(196, 112, 56), 6.0),
        Material::Tin => (Color32::from_rgb(196, 204, 212), 6.0),
        Material::Iron => (Color32::from_rgb(120, 60, 50), 6.0),
    }
}

/// A span of ticks in the largest units that matter, e.g. "2 years, 14 days".
fn format_duration(ticks: u64) -> String {
    let days = ticks / TICKS_PER_DAY;
    let (years, days) = (days / DAYS_PER_YEAR, days % DAYS_PER_YEAR);
    let hours = ticks % TICKS_PER_DAY / 60;
    let unit = |n: u64, name: &str| format!("{n} {name}{}", if n == 1 { "" } else { "s" });
    if years > 0 {
        format!("{}, {}", unit(years, "year"), unit(days, "day"))
    } else if days > 0 {
        format!("{}, {}", unit(days, "day"), unit(hours, "hour"))
    } else {
        unit(hours, "hour")
    }
}

/// Matches the HUD, which counts years and days from 1.
fn format_date(tick: u64) -> String {
    let t = WorldTime { tick };
    format!("year {}, day {}", t.year() + 1, t.day() % DAYS_PER_YEAR + 1)
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

#[cfg(test)]
mod tests {
    use super::*;

    fn view() -> WorldView {
        let person = |id, x, y| PersonView {
            id,
            name: format!("P{id}"),
            x,
            y,
            born_tick: 0,
            knowledge: 0.0,
            acquaintances: 0,
            inventory: vec![],
        };
        let deposit = |id, x, y| DepositView {
            id,
            material: Material::Wood,
            x,
            y,
            amount: 5,
        };
        WorldView {
            tick: 0,
            year: 0,
            day_of_year: 0,
            minute_of_day: 0,
            daylight: 0.0,
            era: Era::Primitive,
            knowledge: 0.0,
            people: vec![person(1, 10.0, 10.0), person(2, 30.0, 10.0)],
            animals: vec![AnimalView {
                id: 3,
                species: Species::Deer,
                x: 14.0,
                y: 10.0,
            }],
            deposits: vec![deposit(4, 12.0, 10.0), deposit(5, 50.0, 10.0)],
            bonds: vec![],
        }
    }

    #[test]
    fn people_and_animals_win_over_deposits() {
        let identity = |x, y| Pos2::new(x, y);
        let v = view();
        // Right on the tree, but the person beside it is still in range.
        assert_eq!(pick(&v, identity, Pos2::new(12.0, 10.0)), Some(1));
        assert_eq!(pick(&v, identity, Pos2::new(51.0, 10.0)), Some(5));
    }

    #[test]
    fn inventory_lists_what_is_carried() {
        assert_eq!(format_inventory(&[]), "Nothing");
        assert_eq!(
            format_inventory(&[(Material::Wood, 3), (Material::Flint, 1)]),
            "3 Wood, 1 Flint"
        );
    }

    #[test]
    fn later_materials_say_when_they_open_up() {
        assert_eq!(gatherable(Material::Stone, Era::Primitive), "Yes");
        assert_eq!(
            gatherable(Material::Copper, Era::Neolithic),
            "From the Bronze Age"
        );
    }

    #[test]
    fn picks_the_nearest_entity_in_range() {
        let identity = |x, y| Pos2::new(x, y);
        let v = view();
        assert_eq!(pick(&v, identity, Pos2::new(11.0, 10.0)), Some(1));
        assert_eq!(pick(&v, identity, Pos2::new(13.0, 10.0)), Some(3));
        assert_eq!(pick(&v, identity, Pos2::new(30.0, 18.0)), Some(2));
        assert_eq!(pick(&v, identity, Pos2::new(100.0, 100.0)), None);
    }

    #[test]
    fn durations_use_the_largest_units() {
        assert_eq!(format_duration(0), "0 hours");
        assert_eq!(format_duration(90), "1 hour");
        assert_eq!(format_duration(TICKS_PER_DAY * 3 + 120), "3 days, 2 hours");
        assert_eq!(
            format_duration(TICKS_PER_DAY * (DAYS_PER_YEAR * 2 + 1)),
            "2 years, 1 day"
        );
    }

    #[test]
    fn dates_count_from_one() {
        assert_eq!(format_date(0), "year 1, day 1");
        assert_eq!(format_date(TICKS_PER_DAY * DAYS_PER_YEAR), "year 2, day 1");
    }
}
