use serde::{Deserialize, Serialize};

/// Technological eras. The world advances when collective knowledge crosses
/// each era's threshold.
#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize)]
pub enum Era {
    Primitive,
    Neolithic,
    BronzeAge,
    IronAge,
    Classical,
    Medieval,
    Renaissance,
    Industrial,
    Modern,
}

impl Era {
    pub const ALL: [Era; 9] = [
        Era::Primitive,
        Era::Neolithic,
        Era::BronzeAge,
        Era::IronAge,
        Era::Classical,
        Era::Medieval,
        Era::Renaissance,
        Era::Industrial,
        Era::Modern,
    ];

    /// Collective knowledge needed to enter this era. Placeholder values, to
    /// be tuned once the simulation has real mechanics.
    pub fn threshold(self) -> f64 {
        match self {
            Era::Primitive => 0.0,
            Era::Neolithic => 1_000.0,
            Era::BronzeAge => 5_000.0,
            Era::IronAge => 20_000.0,
            Era::Classical => 60_000.0,
            Era::Medieval => 150_000.0,
            Era::Renaissance => 400_000.0,
            Era::Industrial => 1_000_000.0,
            Era::Modern => 3_000_000.0,
        }
    }

    pub fn for_knowledge(knowledge: f64) -> Era {
        Era::ALL
            .into_iter()
            .rev()
            .find(|era| knowledge >= era.threshold())
            .unwrap_or(Era::Primitive)
    }

    pub fn name(self) -> &'static str {
        match self {
            Era::Primitive => "Primitive",
            Era::Neolithic => "Neolithic",
            Era::BronzeAge => "Bronze Age",
            Era::IronAge => "Iron Age",
            Era::Classical => "Classical",
            Era::Medieval => "Medieval",
            Era::Renaissance => "Renaissance",
            Era::Industrial => "Industrial",
            Era::Modern => "Modern",
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn thresholds_are_ordered() {
        for pair in Era::ALL.windows(2) {
            assert!(pair[0].threshold() < pair[1].threshold());
        }
    }

    #[test]
    fn era_lookup() {
        assert_eq!(Era::for_knowledge(0.0), Era::Primitive);
        assert_eq!(Era::for_knowledge(999.0), Era::Primitive);
        assert_eq!(Era::for_knowledge(1_000.0), Era::Neolithic);
        assert_eq!(Era::for_knowledge(f64::MAX), Era::Modern);
    }
}
