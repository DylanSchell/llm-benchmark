//! Benchmark categories.
//!
//! A category isolates a benchmark family so its results never collide with
//! another family's. The Exercism/polyglot set is the default (`Polyglot`); the
//! Advent of Code 2015 puzzles are a separate category (`Aoc2015`).

use serde::{Deserialize, Serialize};

/// The benchmark family a result / run belongs to.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum Category {
    /// The existing Exercism exercise set (default).
    Polyglot,
    /// The Advent of Code 2015 puzzle set.
    Aoc2015,
}

impl Default for Category {
    fn default() -> Self {
        Self::Polyglot
    }
}

impl std::fmt::Display for Category {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Category::Polyglot => write!(f, "polyglot"),
            Category::Aoc2015 => write!(f, "aoc2015"),
        }
    }
}

impl Category {
    /// All known categories.
    pub const ALL: &[Category] = &[Category::Polyglot, Category::Aoc2015];

    /// Parse from a string, case-insensitive. Unknown values default to `Polyglot`.
    pub fn parse(s: &str) -> Self {
        match s.to_lowercase().as_str() {
            "aoc2015" | "aoc" | "advent" => Category::Aoc2015,
            _ => Category::Polyglot,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn defaults_to_polyglot() {
        assert_eq!(Category::default(), Category::Polyglot);
    }

    #[test]
    fn serde_round_trips_lowercase() {
        assert_eq!(serde_json::to_string(&Category::Aoc2015).unwrap(), "\"aoc2015\"");
        assert_eq!(serde_json::from_str::<Category>("\"aoc2015\"").unwrap(), Category::Aoc2015);
        assert_eq!(serde_json::from_str::<Category>("\"polyglot\"").unwrap(), Category::Polyglot);
    }

    #[test]
    fn unknown_strings_default_to_polyglot() {
        assert_eq!(Category::parse("garbage"), Category::Polyglot);
        assert_eq!(Category::parse("aoc2015"), Category::Aoc2015);
        assert_eq!(Category::parse("AOC"), Category::Aoc2015);
    }

    #[test]
    fn display_is_lowercase() {
        assert_eq!(Category::Aoc2015.to_string(), "aoc2015");
        assert_eq!(Category::Polyglot.to_string(), "polyglot");
    }
}
