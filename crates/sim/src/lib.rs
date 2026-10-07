//! Deterministic world simulation core.
//!
//! Everything in here is pure logic: no I/O, no clocks, no threads. The server
//! drives [`World::step`] on a timer and persists [`World::to_snapshot`] bytes;
//! given the same seed and the same number of steps, two worlds are identical.

mod era;
mod rng;
mod time;
mod world;

pub use era::Era;
pub use rng::Rng;
pub use time::{WorldTime, DAYS_PER_YEAR, TICKS_PER_DAY};
pub use world::{
    Animal, EntityId, Event, Person, Relationship, SnapshotError, Species, Vec2, World,
    WorldConfig, SNAPSHOT_VERSION,
};
