//! Wire format between the server and the browser client.
//!
//! Messages are JSON text WebSocket frames. [`PROTOCOL_VERSION`] lets the
//! client notice when the server it is talking to has changed shape. Enums
//! are sent as their variant names, e.g. `"BronzeAge"`; the [`Welcome`]
//! message carries a [`Catalog`] describing every name the client will see.
//!
//! [`Welcome`]: ServerMsg::Welcome

use std::collections::{HashMap, HashSet, VecDeque};

use serde::{Deserialize, Serialize};
use sim::{DeathCause, EntityId, Era, Event, Material, Species, World};

pub const PROTOCOL_VERSION: u32 = 9;

/// How many log entries the server remembers for newly connected clients.
pub const LOG_CAPACITY: usize = 2000;

/// Relationships weaker than this are not sent to the client.
const MIN_VISIBLE_AFFINITY: f32 = 0.15;

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub enum ServerMsg {
    /// First message on every connection.
    Welcome {
        protocol_version: u32,
        world_width: f32,
        world_height: f32,
        catalog: Catalog,
    },
    Frame(WorldView),
    /// Log entries the client has not seen yet, oldest first. Sent right after
    /// the frame that announced them (see [`WorldView::log_seq`]).
    Log(Vec<LogEntry>),
}

/// One line of the world's history. The text is written when the event
/// happens, so it still reads correctly after the people in it have died.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct LogEntry {
    /// Increases by one per entry, for the life of the server.
    pub seq: u64,
    pub tick: u64,
    pub text: String,
    /// Everyone the entry is about, so the client can build per-entity logs.
    pub involved: Vec<EntityId>,
}

/// The most recent [`LOG_CAPACITY`] entries.
#[derive(Debug, Default)]
pub struct EventLog {
    entries: VecDeque<LogEntry>,
    next_seq: u64,
    names: HashMap<EntityId, String>,
}

impl EventLog {
    pub fn new(world: &World) -> Self {
        let mut log = Self::default();
        log.remember_names(world);
        log
    }

    /// Records the events of the step that just ran. Call after every step,
    /// with the world as it is afterwards.
    pub fn record(&mut self, world: &World, events: &[Event]) {
        self.remember_names(world);
        for event in events {
            let (text, involved) = self.describe(event);
            self.entries.push_back(LogEntry {
                seq: self.next_seq,
                tick: world.time.tick,
                text,
                involved,
            });
            self.next_seq += 1;
        }
        while self.entries.len() > LOG_CAPACITY {
            self.entries.pop_front();
        }
    }

    /// The sequence number the next entry will get.
    pub fn next_seq(&self) -> u64 {
        self.next_seq
    }

    /// Everything still remembered with a sequence number of `seq` or more.
    pub fn since(&self, seq: u64) -> Vec<LogEntry> {
        self.entries
            .iter()
            .filter(|e| e.seq >= seq)
            .cloned()
            .collect()
    }

    fn remember_names(&mut self, world: &World) {
        for p in &world.people {
            self.names.entry(p.id).or_insert_with(|| p.name.clone());
        }
    }

    fn name(&self, id: EntityId) -> String {
        self.names
            .get(&id)
            .cloned()
            .unwrap_or_else(|| format!("#{id}"))
    }

    fn describe(&self, event: &Event) -> (String, Vec<EntityId>) {
        match *event {
            Event::Met { a, b } => (
                format!("{} and {} met", self.name(a), self.name(b)),
                vec![a, b],
            ),
            Event::Born {
                child,
                parents: (p, q),
            } => (
                format!(
                    "{} was born to {} and {}",
                    self.name(child),
                    self.name(p),
                    self.name(q)
                ),
                vec![child, p, q],
            ),
            Event::Hunted { hunter, species } => (
                format!(
                    "{} killed a {}",
                    self.name(hunter),
                    species.name().to_lowercase()
                ),
                vec![hunter],
            ),
            Event::FocusChanged { person, focus } => (
                format!("{} began {}", self.name(person), focus.description()),
                vec![person],
            ),
            Event::EraReached { era } => (format!("The {} began", era.name()), vec![]),
            Event::Died { person, cause } => (
                format!(
                    "{} died of {}",
                    self.name(person),
                    match cause {
                        DeathCause::Starvation => "starvation",
                        DeathCause::OldAge => "old age",
                        DeathCause::Wolf => "a wolf attack",
                    }
                ),
                vec![person],
            ),
        }
    }
}

/// Static facts about the world's rules, sent once so the client never has
/// to duplicate them.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct Catalog {
    pub ticks_per_day: u64,
    pub days_per_year: u64,
    /// The most, in kilograms, that a person can carry.
    pub carry_limit: u32,
    /// In order, earliest first.
    pub eras: Vec<EraInfo>,
    pub materials: Vec<MaterialInfo>,
    pub species: Vec<SpeciesInfo>,
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct EraInfo {
    pub id: Era,
    pub name: String,
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct MaterialInfo {
    pub id: Material,
    pub name: String,
    /// What a deposit looks like, e.g. "Copper ore".
    pub deposit_name: String,
    /// The first era in which it can be gathered.
    pub era: Era,
    pub forage: bool,
    /// In kilograms.
    pub weight: u32,
    pub capacity: u32,
    pub regrowth_per_day: u32,
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct SpeciesInfo {
    pub id: Species,
    pub name: String,
    pub speed: f32,
}

impl Catalog {
    pub fn new() -> Self {
        Self {
            ticks_per_day: sim::TICKS_PER_DAY,
            days_per_year: sim::DAYS_PER_YEAR,
            carry_limit: sim::CARRY_LIMIT,
            eras: Era::ALL
                .into_iter()
                .map(|id| EraInfo {
                    id,
                    name: id.name().to_owned(),
                })
                .collect(),
            materials: Material::ALL
                .into_iter()
                .map(|id| MaterialInfo {
                    id,
                    name: id.name().to_owned(),
                    deposit_name: id.deposit_name().to_owned(),
                    era: id.era(),
                    forage: id.is_forage(),
                    weight: id.weight(),
                    capacity: id.capacity(),
                    regrowth_per_day: id.regrowth_per_day(),
                })
                .collect(),
            species: Species::ALL
                .into_iter()
                .map(|id| SpeciesInfo {
                    id,
                    name: id.name().to_owned(),
                    speed: id.speed(),
                })
                .collect(),
        }
    }
}

impl Default for Catalog {
    fn default() -> Self {
        Self::new()
    }
}

/// What the client needs to draw one frame. Deliberately smaller than [`World`].
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct WorldView {
    pub tick: u64,
    pub year: u64,
    pub day_of_year: u16,
    pub minute_of_day: u16,
    pub daylight: f32,
    pub era: Era,
    pub knowledge: f64,
    /// [`EventLog::next_seq`] when the frame was made; a client that has seen
    /// fewer entries than this is about to receive a [`ServerMsg::Log`].
    pub log_seq: u64,
    pub people: Vec<PersonView>,
    pub animals: Vec<AnimalView>,
    pub deposits: Vec<DepositView>,
    pub bonds: Vec<BondView>,
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct PersonView {
    pub id: EntityId,
    pub name: String,
    pub x: f32,
    pub y: f32,
    pub born_tick: u64,
    /// `None` for the founders.
    pub parents: Option<(EntityId, EntityId)>,
    pub knowledge: f64,
    /// 0.0 = full, 1.0 = starving.
    pub hunger: f32,
    /// What the person is doing, e.g. "gathering wood".
    pub focus: String,
    /// Everyone this person has ever met, including bonds too weak to send.
    pub acquaintances: u32,
    /// Materials carried, in [`Material`] order. Only those with a count.
    pub inventory: Vec<(Material, u32)>,
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct AnimalView {
    pub id: EntityId,
    pub species: Species,
    pub x: f32,
    pub y: f32,
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct DepositView {
    pub id: EntityId,
    pub material: Material,
    pub x: f32,
    pub y: f32,
    pub amount: u32,
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct BondView {
    pub a: EntityId,
    pub b: EntityId,
    pub affinity: f32,
    pub encounters: u32,
    pub first_met: u64,
}

impl WorldView {
    pub fn new(world: &World, log_seq: u64) -> Self {
        Self {
            log_seq,
            ..Self::from(world)
        }
    }
}

impl From<&World> for WorldView {
    fn from(world: &World) -> Self {
        let t = world.time;
        let living: HashSet<EntityId> = world.people.iter().map(|p| p.id).collect();
        let mut acquaintances: HashMap<EntityId, u32> = HashMap::new();
        for &(a, b) in world.relationships().keys() {
            *acquaintances.entry(a).or_default() += 1;
            *acquaintances.entry(b).or_default() += 1;
        }
        Self {
            tick: t.tick,
            year: t.year(),
            day_of_year: (t.day() % sim::DAYS_PER_YEAR) as u16,
            minute_of_day: t.minute_of_day() as u16,
            daylight: t.daylight(),
            era: world.era,
            knowledge: world.knowledge,
            log_seq: 0,
            people: world
                .people
                .iter()
                .map(|p| PersonView {
                    id: p.id,
                    name: p.name.clone(),
                    x: p.position.x,
                    y: p.position.y,
                    born_tick: p.born_tick,
                    parents: p.parents,
                    knowledge: p.knowledge,
                    hunger: p.hunger,
                    focus: p.focus.description(),
                    acquaintances: acquaintances.get(&p.id).copied().unwrap_or(0),
                    inventory: p.inventory.iter().map(|(&m, &n)| (m, n)).collect(),
                })
                .collect(),
            animals: world
                .animals
                .iter()
                .map(|a| AnimalView {
                    id: a.id,
                    species: a.species,
                    x: a.position.x,
                    y: a.position.y,
                })
                .collect(),
            deposits: world
                .deposits
                .iter()
                .map(|d| DepositView {
                    id: d.id,
                    material: d.material,
                    x: d.position.x,
                    y: d.position.y,
                    amount: d.amount,
                })
                .collect(),
            bonds: world
                .relationships()
                .iter()
                .filter(|(&(a, b), rel)| {
                    rel.affinity >= MIN_VISIBLE_AFFINITY
                        && living.contains(&a)
                        && living.contains(&b)
                })
                .map(|(&(a, b), rel)| BondView {
                    a,
                    b,
                    affinity: rel.affinity,
                    encounters: rel.encounters,
                    first_met: rel.first_met,
                })
                .collect(),
        }
    }
}

pub fn encode(msg: &ServerMsg) -> String {
    serde_json::to_string(msg).expect("ServerMsg is always serializable")
}

pub fn decode(text: &str) -> Result<ServerMsg, serde_json::Error> {
    serde_json::from_str(text)
}

#[cfg(test)]
mod tests {
    use super::*;
    use sim::WorldConfig;

    #[test]
    fn frame_round_trip() {
        let mut world = World::new(9, WorldConfig::default());
        for _ in 0..sim::TICKS_PER_DAY {
            world.step();
        }
        let msg = ServerMsg::Frame(WorldView::from(&world));
        assert_eq!(decode(&encode(&msg)).unwrap(), msg);
    }

    #[test]
    fn log_names_everyone_and_round_trips() {
        let mut world = World::new(9, WorldConfig::default());
        let mut log = EventLog::new(&world);
        for _ in 0..sim::TICKS_PER_DAY * 5 {
            let events = world.step();
            log.record(&world, &events);
        }
        let all = log.since(0);
        assert!(!all.is_empty());
        assert!(all.len() <= LOG_CAPACITY);
        assert!(
            all.iter().all(|e| !e.text.contains('#')),
            "every name known"
        );
        assert_eq!(all.last().unwrap().seq + 1, log.next_seq());
        let msg = ServerMsg::Log(all);
        assert_eq!(decode(&encode(&msg)).unwrap(), msg);
    }

    #[test]
    fn welcome_round_trip() {
        let msg = ServerMsg::Welcome {
            protocol_version: PROTOCOL_VERSION,
            world_width: 100.0,
            world_height: 50.0,
            catalog: Catalog::new(),
        };
        assert_eq!(decode(&encode(&msg)).unwrap(), msg);
    }

    #[test]
    fn enums_are_sent_by_name() {
        let json = encode(&ServerMsg::Welcome {
            protocol_version: PROTOCOL_VERSION,
            world_width: 1.0,
            world_height: 1.0,
            catalog: Catalog::new(),
        });
        assert!(json.contains(r#""id":"BronzeAge""#));
    }

    #[test]
    fn acquaintances_match_relationships() {
        let mut world = World::new(9, WorldConfig::default());
        for _ in 0..sim::TICKS_PER_DAY * 5 {
            world.step();
        }
        let view = WorldView::from(&world);
        let total: u32 = view.people.iter().map(|p| p.acquaintances).sum();
        assert!(
            total > 0,
            "the founding group should meet within a few days"
        );
        assert_eq!(total as usize, 2 * world.relationships().len());
    }
}
