//! Advent of Code 2015 exercise model.
//!
//! An AoC "exercise" is a puzzle day. Unlike an Exercism exercise it has no
//! language, no test suite, and no `.meta` reference implementation. The agent
//! solves it in any language available in the container, then validates its
//! answer against a validator endpoint (see `benchmark-core`'s AoC runner).

use serde::{Deserialize, Serialize};

/// The year AoC 2015 puzzles are for.
pub const AOC_YEAR: u32 = 2015;

/// One Advent of Code 2015 puzzle day.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct AocDay {
    /// Puzzle day, `1..=25`.
    pub day: u32,
    /// The year (always 2015 for this category).
    pub year: u32,
    /// The seed used to generate this day's input. Same `user` → same input,
    /// which is what lets the validator regenerate the exact input the agent saw.
    pub user: String,
}

impl AocDay {
    /// Create a day with the given day number and seed.
    pub fn new(day: u32, user: impl Into<String>) -> Self {
        Self {
            day,
            year: AOC_YEAR,
            user: user.into(),
        }
    }

    /// The result-file exercise name, e.g. `day07`.
    pub fn exercise_name(&self) -> String {
        format!("day{:02}", self.day)
    }

    /// The result-file language value (AoC is language-agnostic).
    pub fn language(&self) -> &'static str {
        "aoc2015"
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn exercise_name_is_zero_padded() {
        assert_eq!(AocDay::new(7, "benchmark").exercise_name(), "day07");
        assert_eq!(AocDay::new(25, "benchmark").exercise_name(), "day25");
    }

    #[test]
    fn language_is_aoc2015() {
        assert_eq!(AocDay::new(1, "benchmark").language(), "aoc2015");
    }
}
