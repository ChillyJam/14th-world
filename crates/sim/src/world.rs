use std::collections::btree_map::Entry;
use std::collections::BTreeMap;

use serde::{Deserialize, Serialize};

use crate::{Deposit, Era, Material, Rng, WorldTime, TICKS_PER_DAY};

pub type EntityId = u64;

/// Bump whenever [`World`]'s serialized layout changes. Snapshots written with
/// an unknown version are refused rather than silently misread; older known
/// versions are migrated in [`World::from_snapshot`].
pub const SNAPSHOT_VERSION: u32 = 3;

const INITIAL_PEOPLE: usize = 4;
/// The world is this many times wider and taller than the original 1024×768.
const WORLD_SCALE: f32 = 10.0;
const INITIAL_ANIMALS: usize = 240;
const PERSON_SPEED: f32 = 0.6;
const WANDER_RADIUS: f32 = 40.0;
const MEET_RADIUS: f32 = 6.0;
/// Minimum ticks between two counted encounters of the same pair.
const ENCOUNTER_COOLDOWN: u64 = 60;
const KNOWLEDGE_PER_ENCOUNTER: f64 = 0.5;
/// Chance per waking tick that a person figures something out on their own.
const DISCOVERY_CHANCE: f32 = 0.002;
/// Hunger gained per waking tick: a full stomach empties in about a day.
const HUNGER_PER_TICK: f32 = 1.0 / TICKS_PER_DAY as f32;
/// Sleeping burns less.
const SLEEP_HUNGER_FACTOR: f32 = 0.5;
/// People start looking for food once they're this hungry.
const FORAGE_THRESHOLD: f32 = 0.5;
/// Chance per waking tick that a foraging person finds something to eat.
const FORAGE_CHANCE: f32 = 0.01;
/// How much hunger one meal takes away.
const MEAL: f32 = 0.4;
/// Chance per tick that someone who is starving dies of it: about a day.
const STARVATION_CHANCE: f32 = 1.0 / TICKS_PER_DAY as f32;
/// People start to die of old age after this many years.
const OLD_AGE_YEARS: u64 = 60;
/// Chance per tick of dying once old: about a month's life left on average.
const OLD_AGE_CHANCE: f32 = 1.0 / (30 * TICKS_PER_DAY) as f32;
/// The most weight, in kilograms, a person can carry.
pub const CARRY_LIMIT: u32 = 40;
/// How close a person has to be to a deposit to gather from it.
const GATHER_RADIUS: f32 = 8.0;
/// Chance per waking tick that a person next to a deposit gathers one unit.
const GATHER_CHANCE: f32 = 0.05;
/// Chance per waking tick that a person in a farming era sows a new field.
const PLANT_CHANCE: f32 = 0.002;
/// Fields each person tends, at most, so farmland stays bounded.
const MAX_FIELDS_PER_PERSON: usize = 2;

#[derive(Clone, Copy, Debug, Default, PartialEq, Serialize, Deserialize)]
pub struct Vec2 {
    pub x: f32,
    pub y: f32,
}

impl Vec2 {
    pub fn new(x: f32, y: f32) -> Self {
        Self { x, y }
    }

    pub fn distance(self, other: Vec2) -> f32 {
        ((self.x - other.x).powi(2) + (self.y - other.y).powi(2)).sqrt()
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Serialize, Deserialize)]
pub struct WorldConfig {
    pub width: f32,
    pub height: f32,
}

impl Default for WorldConfig {
    fn default() -> Self {
        Self {
            width: 1024.0 * WORLD_SCALE,
            height: 768.0 * WORLD_SCALE,
        }
    }
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct Person {
    pub id: EntityId,
    pub name: String,
    pub born_tick: u64,
    /// Where this person sleeps and roams around.
    pub home: Vec2,
    pub position: Vec2,
    pub target: Vec2,
    pub knowledge: f64,
    /// 0.0 = full, 1.0 = starving.
    pub hunger: f32,
    /// Materials this person has gathered and is carrying.
    pub inventory: BTreeMap<Material, u32>,
}

impl Person {
    /// Total weight of everything this person is carrying, in kilograms.
    pub fn carried_weight(&self) -> u32 {
        self.inventory.iter().map(|(m, n)| m.weight() * n).sum()
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub enum Species {
    Deer,
    Rabbit,
    Wolf,
}

impl Species {
    pub const ALL: [Species; 3] = [Species::Deer, Species::Rabbit, Species::Wolf];

    pub fn name(self) -> &'static str {
        match self {
            Species::Deer => "Deer",
            Species::Rabbit => "Rabbit",
            Species::Wolf => "Wolf",
        }
    }

    /// Distance covered per tick.
    pub fn speed(self) -> f32 {
        match self {
            Species::Deer => 0.8,
            Species::Rabbit => 1.0,
            Species::Wolf => 1.2,
        }
    }
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct Animal {
    pub id: EntityId,
    pub species: Species,
    pub position: Vec2,
    pub target: Vec2,
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct Relationship {
    /// 0.0 = strangers, 1.0 = inseparable.
    pub affinity: f32,
    pub encounters: u32,
    pub first_met: u64,
    pub last_met: u64,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum DeathCause {
    Starvation,
    OldAge,
}

/// Something notable that happened during a step. The server appends these to
/// the history log.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum Event {
    Met { a: EntityId, b: EntityId },
    EraReached { era: Era },
    Died { person: EntityId, cause: DeathCause },
}

impl Event {
    pub fn kind(&self) -> &'static str {
        match self {
            Event::Met { .. } => "met",
            Event::EraReached { .. } => "era_reached",
            Event::Died { .. } => "died",
        }
    }
}

#[derive(Debug, thiserror::Error)]
pub enum SnapshotError {
    #[error("snapshot version {found} is not supported (expected {SNAPSHOT_VERSION})")]
    UnsupportedVersion { found: u32 },
    #[error("malformed snapshot: {0}")]
    Malformed(#[from] postcard::Error),
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct World {
    pub config: WorldConfig,
    pub time: WorldTime,
    pub era: Era,
    /// Collective knowledge of everyone who has ever lived.
    pub knowledge: f64,
    pub people: Vec<Person>,
    pub animals: Vec<Animal>,
    pub deposits: Vec<Deposit>,
    /// Keyed by `(lower id, higher id)`.
    relationships: BTreeMap<(EntityId, EntityId), Relationship>,
    rng: Rng,
    next_id: EntityId,
}

impl World {
    pub fn new(seed: u64, config: WorldConfig) -> Self {
        let mut world = Self {
            config,
            time: WorldTime::default(),
            era: Era::Primitive,
            knowledge: 0.0,
            people: Vec::new(),
            animals: Vec::new(),
            deposits: Vec::new(),
            relationships: BTreeMap::new(),
            rng: Rng::new(seed),
            next_id: 1,
        };

        let centre = Vec2::new(config.width / 2.0, config.height / 2.0);
        for _ in 0..INITIAL_PEOPLE {
            let id = world.alloc_id();
            let name = world.random_name();
            let position = Vec2::new(
                centre.x + world.rng.range_f32(-30.0, 30.0),
                centre.y + world.rng.range_f32(-30.0, 30.0),
            );
            world.people.push(Person {
                id,
                name,
                born_tick: 0,
                home: centre,
                position,
                target: position,
                knowledge: 0.0,
                hunger: 0.0,
                inventory: BTreeMap::new(),
            });
        }

        for _ in 0..INITIAL_ANIMALS {
            let id = world.alloc_id();
            let species = match world.rng.below(3) {
                0 => Species::Deer,
                1 => Species::Rabbit,
                _ => Species::Wolf,
            };
            let position = Vec2::new(
                world.rng.range_f32(0.0, config.width),
                world.rng.range_f32(0.0, config.height),
            );
            world.animals.push(Animal {
                id,
                species,
                position,
                target: position,
            });
        }

        world.spawn_deposits(centre);
        world
    }

    /// Scatter material deposits across the map, plus a few within reach of
    /// `home` so the first people have something to gather.
    fn spawn_deposits(&mut self, home: Vec2) {
        for material in Material::ALL {
            self.spawn_material(material, home);
        }
    }

    /// Worlds saved before a material existed get deposits of it laid out
    /// around the founders' home, as in a new world.
    fn add_missing_deposits(&mut self) {
        let home = self.people.first().map_or(
            Vec2::new(self.config.width / 2.0, self.config.height / 2.0),
            |p| p.home,
        );
        for material in Material::ALL {
            if !self.deposits.iter().any(|d| d.material == material) {
                self.spawn_material(material, home);
            }
        }
    }

    fn spawn_material(&mut self, material: Material, home: Vec2) {
        let bounds = self.config;
        for _ in 0..material.near_home() {
            let position = Vec2::new(
                (home.x + self.rng.range_f32(-WANDER_RADIUS, WANDER_RADIUS))
                    .clamp(0.0, bounds.width),
                (home.y + self.rng.range_f32(-WANDER_RADIUS, WANDER_RADIUS))
                    .clamp(0.0, bounds.height),
            );
            self.add_deposit(material, position);
        }
        for _ in 0..material.scattered() * WORLD_SCALE as usize {
            let position = Vec2::new(
                self.rng.range_f32(0.0, bounds.width),
                self.rng.range_f32(0.0, bounds.height),
            );
            self.add_deposit(material, position);
        }
    }

    fn add_deposit(&mut self, material: Material, position: Vec2) {
        let id = self.alloc_id();
        self.deposits.push(Deposit {
            id,
            material,
            position,
            amount: material.capacity(),
        });
    }

    pub fn relationships(&self) -> &BTreeMap<(EntityId, EntityId), Relationship> {
        &self.relationships
    }

    /// Advance the world by one tick (one in-world minute).
    pub fn step(&mut self) -> Vec<Event> {
        self.time.tick += 1;
        let mut events = Vec::new();
        let bounds = self.config;

        // People sleep at night; animals keep roaming.
        let asleep = self.time.is_night();
        for person in &mut self.people {
            let rate = if asleep {
                HUNGER_PER_TICK * SLEEP_HUNGER_FACTOR
            } else {
                HUNGER_PER_TICK
            };
            person.hunger = (person.hunger + rate).min(1.0);
            if !asleep && person.hunger >= FORAGE_THRESHOLD && self.rng.chance(FORAGE_CHANCE) {
                person.hunger = (person.hunger - MEAL).max(0.0);
            }
        }
        self.deaths(&mut events);
        if !asleep {
            for person in &mut self.people {
                wander(
                    &mut person.position,
                    &mut person.target,
                    person.home,
                    PERSON_SPEED,
                    &mut self.rng,
                    bounds,
                );
                if self.rng.chance(DISCOVERY_CHANCE) {
                    person.knowledge += 1.0;
                    self.knowledge += 1.0;
                }
            }
            self.encounters(&mut events);
            self.plant();
            self.gather();
        }

        if self.time.tick.is_multiple_of(TICKS_PER_DAY) {
            for deposit in &mut self.deposits {
                let regrown = deposit.amount + deposit.material.regrowth_per_day();
                deposit.amount = regrown.min(deposit.material.capacity());
            }
        }

        for animal in &mut self.animals {
            let speed = animal.species.speed();
            let anchor = animal.position;
            wander(
                &mut animal.position,
                &mut animal.target,
                anchor,
                speed,
                &mut self.rng,
                bounds,
            );
        }

        let era = Era::for_knowledge(self.knowledge);
        if era > self.era {
            self.era = era;
            events.push(Event::EraReached { era });
        }

        events
    }

    /// The starving and the very old may die. Their relationships remain as
    /// history, but they no longer move, gather or meet anyone.
    fn deaths(&mut self, events: &mut Vec<Event>) {
        let tick = self.time.tick;
        let rng = &mut self.rng;
        self.people.retain(|p| {
            let age_years = (tick - p.born_tick) / (TICKS_PER_DAY * crate::DAYS_PER_YEAR);
            let cause = if p.hunger >= 1.0 && rng.chance(STARVATION_CHANCE) {
                DeathCause::Starvation
            } else if age_years >= OLD_AGE_YEARS && rng.chance(OLD_AGE_CHANCE) {
                DeathCause::OldAge
            } else {
                return true;
            };
            events.push(Event::Died {
                person: p.id,
                cause,
            });
            false
        });
    }

    /// People who come close form or strengthen relationships and learn from
    /// each other. O(n²); replace with a spatial index once populations grow.
    fn encounters(&mut self, events: &mut Vec<Event>) {
        let tick = self.time.tick;
        for i in 0..self.people.len() {
            for j in (i + 1)..self.people.len() {
                if self.people[i].position.distance(self.people[j].position) > MEET_RADIUS {
                    continue;
                }
                let (a, b) = ordered(self.people[i].id, self.people[j].id);
                let counted = match self.relationships.entry((a, b)) {
                    Entry::Vacant(slot) => {
                        slot.insert(Relationship {
                            affinity: 0.1,
                            encounters: 1,
                            first_met: tick,
                            last_met: tick,
                        });
                        events.push(Event::Met { a, b });
                        true
                    }
                    Entry::Occupied(mut slot) => {
                        let rel = slot.get_mut();
                        if tick - rel.last_met >= ENCOUNTER_COOLDOWN {
                            rel.encounters += 1;
                            rel.affinity = (rel.affinity + 0.05).min(1.0);
                            rel.last_met = tick;
                            true
                        } else {
                            false
                        }
                    }
                };
                if counted {
                    self.people[i].knowledge += KNOWLEDGE_PER_ENCOUNTER;
                    self.people[j].knowledge += KNOWLEDGE_PER_ENCOUNTER;
                    self.knowledge += 2.0 * KNOWLEDGE_PER_ENCOUNTER;
                }
            }
        }
    }

    /// Once the world knows farming, people sow fields near where they are.
    /// A new field starts bare and ripens a little each day.
    fn plant(&mut self) {
        if self.era < Material::Grain.era() {
            return;
        }
        let bounds = self.config;
        let mut fields = self
            .deposits
            .iter()
            .filter(|d| d.material == Material::Grain)
            .count();
        for i in 0..self.people.len() {
            if fields >= MAX_FIELDS_PER_PERSON * self.people.len() {
                break;
            }
            if !self.rng.chance(PLANT_CHANCE) {
                continue;
            }
            let here = self.people[i].position;
            let position = Vec2::new(
                here.x.clamp(0.0, bounds.width),
                here.y.clamp(0.0, bounds.height),
            );
            self.add_deposit(Material::Grain, position);
            if let Some(field) = self.deposits.last_mut() {
                field.amount = 0;
            }
            fields += 1;
        }
    }

    /// People standing next to a deposit their era can work sometimes take a
    /// unit from it, if they can still carry its weight. When several are in
    /// reach they use the nearest.
    fn gather(&mut self) {
        let era = self.era;
        for person in &mut self.people {
            if !self.rng.chance(GATHER_CHANCE) {
                continue;
            }
            let here = person.position;
            let load = person.carried_weight();
            let nearest = self
                .deposits
                .iter_mut()
                .filter(|d| {
                    d.amount > 0
                        && d.material.era() <= era
                        && load + d.material.weight() <= CARRY_LIMIT
                })
                .map(|d| (d.position.distance(here), d))
                .filter(|(dist, _)| *dist <= GATHER_RADIUS)
                .min_by(|a, b| a.0.total_cmp(&b.0));
            if let Some((_, deposit)) = nearest {
                deposit.amount -= 1;
                *person.inventory.entry(deposit.material).or_default() += 1;
            }
        }
    }

    pub fn to_snapshot(&self) -> Result<Vec<u8>, SnapshotError> {
        Ok(postcard::to_stdvec(&(SNAPSHOT_VERSION, self))?)
    }

    pub fn from_snapshot(bytes: &[u8]) -> Result<Self, SnapshotError> {
        let (version, rest): (u32, &[u8]) = postcard::take_from_bytes(bytes)?;
        match version {
            1 => Ok(postcard::from_bytes::<v1::World>(rest)?.into()),
            2 => {
                let mut world: Self = postcard::from_bytes::<v2::World>(rest)?.into();
                world.add_missing_deposits();
                Ok(world)
            }
            SNAPSHOT_VERSION => {
                let mut world: Self = postcard::from_bytes(rest)?;
                world.add_missing_deposits();
                Ok(world)
            }
            found => Err(SnapshotError::UnsupportedVersion { found }),
        }
    }

    fn alloc_id(&mut self) -> EntityId {
        let id = self.next_id;
        self.next_id += 1;
        id
    }

    fn random_name(&mut self) -> String {
        const SYLLABLES: [&str; 12] = [
            "ka", "ru", "mi", "to", "an", "el", "zu", "ri", "ba", "no", "sa", "ul",
        ];
        let len = 2 + self.rng.below(2);
        let mut name: String = (0..len)
            .map(|_| SYLLABLES[self.rng.below(SYLLABLES.len() as u64) as usize])
            .collect();
        name[..1].make_ascii_uppercase();
        name
    }
}

/// The snapshot layout before materials existed, kept so worlds saved by
/// older servers carry on where they left off.
mod v1 {
    use super::*;

    #[derive(Serialize, Deserialize)]
    pub struct Person {
        pub id: EntityId,
        pub name: String,
        pub born_tick: u64,
        pub home: Vec2,
        pub position: Vec2,
        pub target: Vec2,
        pub knowledge: f64,
    }

    #[derive(Serialize, Deserialize)]
    pub struct World {
        pub config: WorldConfig,
        pub time: WorldTime,
        pub era: Era,
        pub knowledge: f64,
        pub people: Vec<Person>,
        pub animals: Vec<Animal>,
        pub relationships: BTreeMap<(EntityId, EntityId), Relationship>,
        pub rng: Rng,
        pub next_id: EntityId,
    }

    /// Everyone starts empty-handed, and deposits are laid out around the
    /// founders' home just as in a new world.
    impl From<World> for super::World {
        fn from(old: World) -> Self {
            let centre = Vec2::new(old.config.width / 2.0, old.config.height / 2.0);
            let home = old.people.first().map_or(centre, |p| p.home);
            let mut world = Self {
                config: old.config,
                time: old.time,
                era: old.era,
                knowledge: old.knowledge,
                people: old
                    .people
                    .into_iter()
                    .map(|p| super::Person {
                        id: p.id,
                        name: p.name,
                        born_tick: p.born_tick,
                        home: p.home,
                        position: p.position,
                        target: p.target,
                        knowledge: p.knowledge,
                        hunger: 0.0,
                        inventory: BTreeMap::new(),
                    })
                    .collect(),
                animals: old.animals,
                deposits: Vec::new(),
                relationships: old.relationships,
                rng: old.rng,
                next_id: old.next_id,
            };
            world.spawn_deposits(home);
            world
        }
    }
}

/// The snapshot layout before hunger existed.
mod v2 {
    use super::*;

    #[derive(Serialize, Deserialize)]
    pub struct Person {
        pub id: EntityId,
        pub name: String,
        pub born_tick: u64,
        pub home: Vec2,
        pub position: Vec2,
        pub target: Vec2,
        pub knowledge: f64,
        pub inventory: BTreeMap<Material, u32>,
    }

    #[derive(Serialize, Deserialize)]
    pub struct World {
        pub config: WorldConfig,
        pub time: WorldTime,
        pub era: Era,
        pub knowledge: f64,
        pub people: Vec<Person>,
        pub animals: Vec<Animal>,
        pub deposits: Vec<Deposit>,
        pub relationships: BTreeMap<(EntityId, EntityId), Relationship>,
        pub rng: Rng,
        pub next_id: EntityId,
    }

    /// Everyone starts well fed.
    impl From<World> for super::World {
        fn from(old: World) -> Self {
            Self {
                config: old.config,
                time: old.time,
                era: old.era,
                knowledge: old.knowledge,
                people: old
                    .people
                    .into_iter()
                    .map(|p| super::Person {
                        id: p.id,
                        name: p.name,
                        born_tick: p.born_tick,
                        home: p.home,
                        position: p.position,
                        target: p.target,
                        knowledge: p.knowledge,
                        hunger: 0.0,
                        inventory: p.inventory,
                    })
                    .collect(),
                animals: old.animals,
                deposits: old.deposits,
                relationships: old.relationships,
                rng: old.rng,
                next_id: old.next_id,
            }
        }
    }
}

fn ordered(a: EntityId, b: EntityId) -> (EntityId, EntityId) {
    if a < b {
        (a, b)
    } else {
        (b, a)
    }
}

/// Walk towards `target`; on arrival pick a new one near `anchor`.
fn wander(
    pos: &mut Vec2,
    target: &mut Vec2,
    anchor: Vec2,
    speed: f32,
    rng: &mut Rng,
    bounds: WorldConfig,
) {
    let dist = pos.distance(*target);
    if dist <= speed {
        *pos = *target;
        *target = Vec2::new(
            (anchor.x + rng.range_f32(-WANDER_RADIUS, WANDER_RADIUS)).clamp(0.0, bounds.width),
            (anchor.y + rng.range_f32(-WANDER_RADIUS, WANDER_RADIUS)).clamp(0.0, bounds.height),
        );
    } else {
        pos.x += (target.x - pos.x) / dist * speed;
        pos.y += (target.y - pos.y) / dist * speed;
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn world() -> World {
        World::new(1235, WorldConfig::default())
    }

    #[test]
    fn starts_small_and_primitive() {
        let w = world();
        assert_eq!(w.people.len(), INITIAL_PEOPLE);
        assert!(w.people.iter().all(|p| p.knowledge == 0.0));
        assert!(w.people.iter().all(|p| p.hunger == 0.0));
        assert_eq!(w.era, Era::Primitive);
    }

    #[test]
    fn deterministic() {
        let mut a = world();
        let mut b = world();
        for _ in 0..TICKS_PER_DAY * 3 {
            assert_eq!(a.step(), b.step());
        }
        assert_eq!(a, b);
    }

    #[test]
    fn people_sleep_at_night() {
        let mut w = world();
        // Tick 0 is midnight.
        let before: Vec<_> = w.people.iter().map(|p| p.position).collect();
        w.step();
        let after: Vec<_> = w.people.iter().map(|p| p.position).collect();
        assert_eq!(before, after);
    }

    #[test]
    fn people_meet_each_other() {
        let mut w = world();
        let mut met = 0;
        for _ in 0..TICKS_PER_DAY * 5 {
            met += w
                .step()
                .iter()
                .filter(|e| matches!(e, Event::Met { .. }))
                .count();
        }
        assert!(met > 0, "the founding group should meet within a few days");
        assert_eq!(met, w.relationships().len());
    }

    #[test]
    fn people_get_hungry_and_eat() {
        let mut w = world();
        for _ in 0..TICKS_PER_DAY / 2 {
            w.step();
        }
        assert!(
            w.people.iter().all(|p| p.hunger > 0.0),
            "half a day without food should make everyone hungry"
        );
        let mut peak: f32 = 0.0;
        for _ in 0..TICKS_PER_DAY * 10 {
            w.step();
            peak = w.people.iter().map(|p| p.hunger).fold(peak, f32::max);
            assert!(w.people.iter().all(|p| (0.0..=1.0).contains(&p.hunger)));
        }
        assert!(peak < 1.0, "foraging should keep people from starving");
    }

    #[test]
    fn the_starving_die() {
        let mut w = world();
        // Nothing to eat: tick 1 is at night, so nobody forages.
        for p in &mut w.people {
            p.hunger = 1.0;
        }
        let mut died = Vec::new();
        for _ in 0..TICKS_PER_DAY * 3 {
            for p in &mut w.people {
                p.hunger = 1.0;
            }
            for e in w.step() {
                if let Event::Died { person, cause } = e {
                    assert_eq!(cause, DeathCause::Starvation);
                    died.push(person);
                }
            }
        }
        assert!(!died.is_empty(), "starving people should die");
        assert_eq!(w.people.len() + died.len(), INITIAL_PEOPLE);
        assert!(w.people.iter().all(|p| !died.contains(&p.id)));
    }

    #[test]
    fn the_old_die() {
        let mut w = world();
        w.time.tick = OLD_AGE_YEARS * crate::DAYS_PER_YEAR * TICKS_PER_DAY;
        for _ in 0..TICKS_PER_DAY * 365 {
            for p in &mut w.people {
                p.hunger = 0.0;
            }
            w.step();
        }
        assert!(w.people.is_empty(), "everyone should have died of old age");
    }

    #[test]
    fn the_young_do_not_die_of_old_age() {
        let mut w = world();
        for _ in 0..TICKS_PER_DAY * 20 {
            for p in &mut w.people {
                p.hunger = 0.0;
            }
            w.step();
        }
        assert_eq!(w.people.len(), INITIAL_PEOPLE);
    }

    #[test]
    fn entities_stay_in_bounds() {
        let mut w = world();
        for _ in 0..TICKS_PER_DAY * 2 {
            w.step();
        }
        let in_bounds = |p: Vec2| {
            (0.0..=w.config.width).contains(&p.x) && (0.0..=w.config.height).contains(&p.y)
        };
        assert!(w.people.iter().all(|p| in_bounds(p.position)));
        assert!(w.animals.iter().all(|a| in_bounds(a.position)));
    }

    #[test]
    fn snapshot_round_trip_resumes_identically() {
        let mut w = world();
        for _ in 0..TICKS_PER_DAY {
            w.step();
        }
        let mut restored = World::from_snapshot(&w.to_snapshot().unwrap()).unwrap();
        assert_eq!(restored, w);
        for _ in 0..TICKS_PER_DAY {
            assert_eq!(restored.step(), w.step());
        }
    }

    #[test]
    fn rejects_unknown_snapshot_version() {
        let bytes = postcard::to_stdvec(&(SNAPSHOT_VERSION + 1, world())).unwrap();
        assert!(matches!(
            World::from_snapshot(&bytes),
            Err(SnapshotError::UnsupportedVersion { .. })
        ));
    }

    #[test]
    fn new_world_has_every_material() {
        let w = world();
        for material in Material::ALL.into_iter().filter(|m| !m.is_planted()) {
            assert!(w.deposits.iter().any(|d| d.material == material));
        }
        let home = w.people[0].home;
        assert!(
            w.deposits
                .iter()
                .any(|d| d.material == Material::Wood
                    && d.position.distance(home) < 2.0 * WANDER_RADIUS),
            "the founders should have trees nearby"
        );
    }

    #[test]
    fn people_gather_only_what_their_era_can_work() {
        let total = |w: &World, m: Material| -> u32 {
            let held: u32 = w.people.iter().filter_map(|p| p.inventory.get(&m)).sum();
            let left: u32 = w
                .deposits
                .iter()
                .filter(|d| d.material == m)
                .map(|d| d.amount)
                .sum();
            held + left
        };
        let mut w = world();
        let before: Vec<u32> = Material::ALL.iter().map(|&m| total(&w, m)).collect();
        // Stop just before the first regrowth, so totals are conserved.
        for _ in 0..TICKS_PER_DAY - 1 {
            w.step();
        }
        assert_eq!(w.era, Era::Primitive);
        let gathered: u32 = w.people.iter().flat_map(|p| p.inventory.values()).sum();
        assert!(
            gathered > 0,
            "the founders should gather something on day one"
        );
        for p in &w.people {
            assert!(p.inventory.keys().all(|m| m.era() == Era::Primitive));
        }
        let after: Vec<u32> = Material::ALL.iter().map(|&m| total(&w, m)).collect();
        assert_eq!(
            before, after,
            "gathering moves materials, never creates them"
        );
    }

    #[test]
    fn people_sow_and_harvest_fields_once_farming_is_known() {
        let mut w = world();
        for _ in 0..TICKS_PER_DAY * 3 {
            w.step();
        }
        assert!(!w.deposits.iter().any(|d| d.material == Material::Grain));

        w.knowledge = Era::Neolithic.threshold();
        // Founders are weighed down by now; empty their hands so the harvest is theirs.
        for p in &mut w.people {
            p.inventory.clear();
        }
        for _ in 0..TICKS_PER_DAY * 10 {
            w.step();
        }
        assert!(w.era >= Era::Neolithic);
        let fields = w
            .deposits
            .iter()
            .filter(|d| d.material == Material::Grain)
            .count();
        assert!(fields > 0, "people should have sown fields");
        assert!(fields <= MAX_FIELDS_PER_PERSON * w.people.len());
        let grain: u32 = w
            .people
            .iter()
            .filter_map(|p| p.inventory.get(&Material::Grain))
            .sum();
        assert!(grain > 0, "people should have harvested grain");
    }

    #[test]
    fn people_never_carry_more_than_the_limit() {
        let mut w = world();
        w.knowledge = Era::Neolithic.threshold();
        for _ in 0..TICKS_PER_DAY * 10 {
            w.step();
            assert!(w.people.iter().all(|p| p.carried_weight() <= CARRY_LIMIT));
        }
        assert!(
            w.people.iter().any(|p| p.carried_weight() > 0),
            "people should still gather"
        );
    }

    #[test]
    fn trees_grow_back_but_rocks_do_not() {
        let mut w = world();
        for d in &mut w.deposits {
            d.amount = 0;
        }
        for _ in 0..TICKS_PER_DAY {
            w.step();
        }
        for d in &w.deposits {
            assert_eq!(d.amount, d.material.regrowth_per_day(), "{:?}", d.material);
        }
    }

    #[test]
    fn migrates_v1_snapshots() {
        let mut w = world();
        for _ in 0..TICKS_PER_DAY {
            w.step();
        }
        let old = v1::World {
            config: w.config,
            time: w.time,
            era: w.era,
            knowledge: w.knowledge,
            people: w
                .people
                .iter()
                .map(|p| v1::Person {
                    id: p.id,
                    name: p.name.clone(),
                    born_tick: p.born_tick,
                    home: p.home,
                    position: p.position,
                    target: p.target,
                    knowledge: p.knowledge,
                })
                .collect(),
            animals: w.animals.clone(),
            relationships: w.relationships.clone(),
            rng: w.rng.clone(),
            next_id: w.next_id,
        };
        let bytes = postcard::to_stdvec(&(1u32, old)).unwrap();
        let mut migrated = World::from_snapshot(&bytes).unwrap();

        assert_eq!(migrated.time, w.time);
        assert_eq!(migrated.relationships, w.relationships);
        assert!(migrated.people.iter().all(|p| p.inventory.is_empty()));
        for material in Material::ALL.into_iter().filter(|m| !m.is_planted()) {
            assert!(migrated.deposits.iter().any(|d| d.material == material));
        }
        let ids: std::collections::BTreeSet<_> = migrated
            .people
            .iter()
            .map(|p| p.id)
            .chain(migrated.animals.iter().map(|a| a.id))
            .chain(migrated.deposits.iter().map(|d| d.id))
            .collect();
        let entities = migrated.people.len() + migrated.animals.len() + migrated.deposits.len();
        assert_eq!(ids.len(), entities, "deposit ids must not clash");
        migrated.step();
    }

    #[test]
    fn snapshots_without_forage_get_it_added() {
        let mut w = world();
        w.deposits.retain(|d| !d.material.is_forage());
        let loaded = World::from_snapshot(&w.to_snapshot().unwrap()).unwrap();
        for material in Material::ALL.into_iter().filter(|m| !m.is_planted()) {
            assert!(loaded.deposits.iter().any(|d| d.material == material));
        }
    }
}
