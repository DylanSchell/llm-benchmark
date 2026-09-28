//! Advent of Code 2015 generator/validator, vendored as a library.
//!
//! The generator/validator logic is copied verbatim from the standalone `aoc2015`
//! crate (see `docs/specs/aoc2015-benchmark.md`). It is pure-`std` and dependency-free
//! (only `std::collections` / `std::cmp`, plus the internal `rng`, `md5`, `json`
//! helpers). It is **not** exposed to benchmark agents — agents only receive the
//! generated input, the description, and instructions to call a validator endpoint.
//!
//! The puzzle descriptions (both parts) are embedded via [`rust_embed`] from the
//! `descriptions/` directory. They are public Advent of Code puzzle text.

mod days;
mod json;
mod rng;

use rust_embed::Embed;

/// Embedded puzzle descriptions (`descriptions/dayNN.md`).
#[derive(Embed)]
#[folder = "descriptions"]
struct Descriptions;

/// A puzzle day: generate a fresh valid input and solve it (returns part 1, part 2).
pub use days::Puzzle;

/// Generate a deterministic input for `(user, year, day)`.
///
/// Same `user` → same input, which is what lets the validator regenerate the exact
/// input the agent was given. `year` must be 2015, `day` in `1..=25`.
pub fn generate(user: &str, year: u32, day: u32) -> String {
    days::generate(user, year, day)
}

/// Solve the puzzle for `(user, year, day)`, returning `(part1, part2)`.
///
/// Day 25 has no part 2, so its second element is empty.
pub fn solve(user: &str, year: u32, day: u32) -> (String, String) {
    days::solve(user, year, day)
}

/// The puzzle description for `day` (both parts, as Markdown).
///
/// Days 1–24 contain part 1 and part 2; day 25 contains part 1 only.
pub fn description(day: u32) -> Option<String> {
    let file = format!("day{day:02}.md");
    let bytes = Descriptions::get(&file)?;
    Some(String::from_utf8_lossy(&bytes.data).into_owned())
}

/// All puzzle days, `1..=25`.
pub fn days() -> Vec<u32> {
    (1..=25).collect()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn days_are_one_through_twenty_five() {
        assert_eq!(days(), (1..=25).collect::<Vec<u32>>());
    }

    #[test]
    fn generate_is_deterministic_for_a_fixed_user() {
        assert_eq!(generate("benchmark", 2015, 1), generate("benchmark", 2015, 1));
        assert_eq!(generate("alice", 2015, 7), generate("alice", 2015, 7));
    }

    #[test]
    fn generate_differs_by_user_and_day() {
        assert_ne!(generate("benchmark", 2015, 1), generate("other", 2015, 1));
        assert_ne!(generate("benchmark", 2015, 1), generate("benchmark", 2015, 2));
    }

    #[test]
    fn solve_round_trips_a_generated_input() {
        // The solver consumes the generated input, so it must not panic and must
        // produce a part-1 answer for every day.
        for day in days() {
            let (p1, _p2) = solve("benchmark", 2015, day);
            assert!(!p1.is_empty(), "day {day} produced an empty part 1");
        }
    }

    /// The solver must never emit a sentinel value indicating "no solution found".
    /// Some generators produce inputs a search cannot solve (e.g. day 22 with high
    /// boss damage), and the solver returns i64::MAX/i64::MIN/usize::MAX in that
    /// case — a value an agent can never match, making the day unsolvable. This
    /// samples many seeds and asserts no sentinel leaks for any day.
    #[test]
    #[ignore] // thorough sweep; slow in debug (day 4/22). Run in release/CI.
    fn solver_never_emits_no_solution_sentinels() {
        // Only the extreme "no solution found" sentinels. `-1` is a legitimate
        // answer for several days (e.g. day 1 floor), so it is not included.
        const SENTINELS: [&str; 3] = [
            "9223372036854775807",  // i64::MAX
            "-9223372036854775808", // i64::MIN
            "18446744073709551615", // u64::MAX
        ];
        for day in days() {
            for i in 0..10u64 {
                let user = format!("probe{i}");
                let (p1, p2) = solve(&user, 2015, day);
                for v in [p1.as_str(), p2.as_str()] {
                    if v.is_empty() {
                        continue; // day 25 has no part 2
                    }
                    assert!(
                        !SENTINELS.contains(&v),
                        "day {day} seed {user} emitted sentinel {v}"
                    );
                }
            }
        }
    }

    #[test]
    fn day25_has_no_part_two() {
        let (_p1, p2) = solve("benchmark", 2015, 25);
        assert!(p2.is_empty(), "day 25 must have no part 2");
    }

    #[test]
    fn descriptions_contain_both_parts() {
        for day in 1..=24 {
            let desc = description(day).expect("description present");
            assert!(
                desc.contains("Part Two") || desc.contains("Part 2"),
                "day {day} description is missing part 2"
            );
        }
        let day25 = description(25).expect("day 25 description present");
        assert!(
            !day25.contains("Part Two") && !day25.contains("Part 2"),
            "day 25 must not have a part 2"
        );
    }

    #[test]
    fn description_is_absent_for_invalid_day() {
        assert!(description(0).is_none());
        assert!(description(26).is_none());
    }
}
