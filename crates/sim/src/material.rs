use serde::{Deserialize, Serialize};

use crate::{EntityId, Era, Vec2};

/// Raw materials found in the world. People can gather a material once their
/// world has reached the era that knows how to work it.
#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize)]
pub enum Material {
    Wood,
    Stone,
    Flint,
    Clay,
    Copper,
    Tin,
    Iron,
    // Foraged food. Appended last so saved worlds keep their material ids.
    Berries,
    Mushrooms,
    Roots,
}

impl Material {
    pub const ALL: [Material; 10] = [
        Material::Wood,
        Material::Stone,
        Material::Flint,
        Material::Clay,
        Material::Copper,
        Material::Tin,
        Material::Iron,
        Material::Berries,
        Material::Mushrooms,
        Material::Roots,
    ];

    pub fn name(self) -> &'static str {
        match self {
            Material::Wood => "Wood",
            Material::Stone => "Stone",
            Material::Flint => "Flint",
            Material::Clay => "Clay",
            Material::Copper => "Copper",
            Material::Tin => "Tin",
            Material::Iron => "Iron",
            Material::Berries => "Berries",
            Material::Mushrooms => "Mushrooms",
            Material::Roots => "Roots",
        }
    }

    /// What a deposit of this material looks like, e.g. "Copper ore".
    pub fn deposit_name(self) -> &'static str {
        match self {
            Material::Wood => "Tree",
            Material::Stone => "Rock",
            Material::Flint => "Flint nodules",
            Material::Clay => "Clay bed",
            Material::Copper => "Copper ore",
            Material::Tin => "Tin ore",
            Material::Iron => "Iron ore",
            Material::Berries => "Berry bush",
            Material::Mushrooms => "Mushroom patch",
            Material::Roots => "Root patch",
        }
    }

    /// The first era in which people can gather this material.
    pub fn era(self) -> Era {
        match self {
            Material::Wood
            | Material::Stone
            | Material::Flint
            | Material::Berries
            | Material::Mushrooms
            | Material::Roots => Era::Primitive,
            Material::Clay => Era::Neolithic,
            Material::Copper | Material::Tin => Era::BronzeAge,
            Material::Iron => Era::IronAge,
        }
    }

    /// Whether this is food gathered by foraging rather than a raw material.
    pub fn is_forage(self) -> bool {
        matches!(
            self,
            Material::Berries | Material::Mushrooms | Material::Roots
        )
    }

    /// Units a fresh deposit holds.
    pub fn capacity(self) -> u32 {
        match self {
            Material::Wood => 20,
            Material::Stone => 60,
            Material::Flint => 15,
            Material::Clay => 40,
            Material::Copper | Material::Tin => 30,
            Material::Iron => 50,
            Material::Berries => 12,
            Material::Mushrooms => 8,
            Material::Roots => 10,
        }
    }

    /// Units a deposit regains each day. Trees and foraged plants grow back.
    pub fn regrowth_per_day(self) -> u32 {
        match self {
            Material::Wood | Material::Roots => 1,
            Material::Berries | Material::Mushrooms => 2,
            _ => 0,
        }
    }

    /// How many deposits a new world scatters across the map. Common
    /// materials first, ores rarer.
    pub(crate) fn scattered(self) -> usize {
        match self {
            Material::Wood => 40,
            Material::Stone => 16,
            Material::Flint => 8,
            Material::Clay => 10,
            Material::Copper => 5,
            Material::Tin => 3,
            Material::Iron => 5,
            Material::Berries => 20,
            Material::Mushrooms => 12,
            Material::Roots => 12,
        }
    }

    /// How many deposits a new world places within reach of the founders'
    /// home, so they have something to gather from day one.
    pub(crate) fn near_home(self) -> usize {
        match self {
            Material::Wood => 3,
            Material::Stone | Material::Flint => 1,
            Material::Berries | Material::Mushrooms | Material::Roots => 2,
            _ => 0,
        }
    }
}

/// A place a material can be gathered from: a tree, a rock, an ore vein, …
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct Deposit {
    pub id: EntityId,
    pub material: Material,
    pub position: Vec2,
    /// Units left. A deposit at zero is exhausted until it regrows, if ever.
    pub amount: u32,
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn primitive_people_have_something_to_gather() {
        assert!(Material::ALL
            .iter()
            .any(|m| m.era() == Era::Primitive && m.near_home() > 0));
    }

    #[test]
    fn every_material_appears_in_a_new_world() {
        assert!(Material::ALL.iter().all(|m| m.scattered() > 0));
    }
}
