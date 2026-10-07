use std::f32::consts::TAU;

use serde::{Deserialize, Serialize};

/// One tick is one in-world minute.
pub const TICKS_PER_DAY: u64 = 24 * 60;
pub const DAYS_PER_YEAR: u64 = 365;

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, PartialOrd, Ord, Serialize, Deserialize)]
pub struct WorldTime {
    pub tick: u64,
}

impl WorldTime {
    pub fn day(self) -> u64 {
        self.tick / TICKS_PER_DAY
    }

    pub fn year(self) -> u64 {
        self.day() / DAYS_PER_YEAR
    }

    pub fn minute_of_day(self) -> u64 {
        self.tick % TICKS_PER_DAY
    }

    /// Sun brightness: 0.0 at midnight, 1.0 at noon.
    pub fn daylight(self) -> f32 {
        let phase = self.minute_of_day() as f32 / TICKS_PER_DAY as f32;
        0.5 - 0.5 * (phase * TAU).cos()
    }

    /// Roughly 20:00 to 04:00.
    pub fn is_night(self) -> bool {
        self.daylight() < 0.25
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn calendar() {
        let t = WorldTime {
            tick: TICKS_PER_DAY * DAYS_PER_YEAR + TICKS_PER_DAY * 3 + 90,
        };
        assert_eq!(t.year(), 1);
        assert_eq!(t.day(), DAYS_PER_YEAR + 3);
        assert_eq!(t.minute_of_day(), 90);
    }

    #[test]
    fn day_night_cycle() {
        let midnight = WorldTime { tick: 0 };
        let noon = WorldTime {
            tick: TICKS_PER_DAY / 2,
        };
        assert!(midnight.daylight() < 0.01);
        assert!(noon.daylight() > 0.99);
        assert!(midnight.is_night());
        assert!(!noon.is_night());
    }
}
