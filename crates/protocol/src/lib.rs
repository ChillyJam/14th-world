//! Wire format between the server and the native client.
//!
//! Messages are [postcard]-encoded binary WebSocket frames. Both sides depend on
//! this crate, so a mismatch is a compile error rather than a runtime surprise;
//! [`PROTOCOL_VERSION`] guards against a client built from an older version.

use serde::{Deserialize, Serialize};
use sim::{EntityId, Era, Species, World};

pub const PROTOCOL_VERSION: u32 = 1;

/// Relationships weaker than this are not sent to the client.
const MIN_VISIBLE_AFFINITY: f32 = 0.15;

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub enum ServerMsg {
    /// First message on every connection.
    Welcome {
        protocol_version: u32,
        world_width: f32,
        world_height: f32,
    },
    Frame(WorldView),
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
    pub people: Vec<PersonView>,
    pub animals: Vec<AnimalView>,
    pub bonds: Vec<BondView>,
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct PersonView {
    pub id: EntityId,
    pub name: String,
    pub x: f32,
    pub y: f32,
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct AnimalView {
    pub id: EntityId,
    pub species: Species,
    pub x: f32,
    pub y: f32,
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct BondView {
    pub a: EntityId,
    pub b: EntityId,
    pub affinity: f32,
}

impl From<&World> for WorldView {
    fn from(world: &World) -> Self {
        let t = world.time;
        Self {
            tick: t.tick,
            year: t.year(),
            day_of_year: (t.day() % sim::DAYS_PER_YEAR) as u16,
            minute_of_day: t.minute_of_day() as u16,
            daylight: t.daylight(),
            era: world.era,
            knowledge: world.knowledge,
            people: world
                .people
                .iter()
                .map(|p| PersonView {
                    id: p.id,
                    name: p.name.clone(),
                    x: p.position.x,
                    y: p.position.y,
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
            bonds: world
                .relationships()
                .iter()
                .filter(|(_, rel)| rel.affinity >= MIN_VISIBLE_AFFINITY)
                .map(|(&(a, b), rel)| BondView {
                    a,
                    b,
                    affinity: rel.affinity,
                })
                .collect(),
        }
    }
}

pub fn encode(msg: &ServerMsg) -> Vec<u8> {
    postcard::to_stdvec(msg).expect("ServerMsg is always serializable")
}

pub fn decode(bytes: &[u8]) -> Result<ServerMsg, postcard::Error> {
    postcard::from_bytes(bytes)
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
}
