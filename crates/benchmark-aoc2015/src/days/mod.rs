pub mod day01;
pub mod day02;
pub mod day03;
pub mod day04;
pub mod day05;
pub mod day06;
pub mod day07;
pub mod day08;
pub mod day09;
pub mod day10;
pub mod day11;
pub mod day12;
pub mod day13;
pub mod day14;
pub mod day15;
pub mod day16;
pub mod day17;
pub mod day18;
pub mod day19;
pub mod day20;
pub mod day21;
pub mod day22;
pub mod day23;
pub mod day24;
pub mod day25;

use crate::rng::{seed, Rng};

/// A puzzle: generate a fresh valid input from an RNG, and solve it.
pub struct Puzzle {
    pub generate: fn(&mut Rng) -> String,
    pub solve: fn(&str) -> (String, String),
}

const PUZZLES: [Puzzle; 25] = [
    Puzzle { generate: day01::generate, solve: day01::solve },
    Puzzle { generate: day02::generate, solve: day02::solve },
    Puzzle { generate: day03::generate, solve: day03::solve },
    Puzzle { generate: day04::generate, solve: day04::solve },
    Puzzle { generate: day05::generate, solve: day05::solve },
    Puzzle { generate: day06::generate, solve: day06::solve },
    Puzzle { generate: day07::generate, solve: day07::solve },
    Puzzle { generate: day08::generate, solve: day08::solve },
    Puzzle { generate: day09::generate, solve: day09::solve },
    Puzzle { generate: day10::generate, solve: day10::solve },
    Puzzle { generate: day11::generate, solve: day11::solve },
    Puzzle { generate: day12::generate, solve: day12::solve },
    Puzzle { generate: day13::generate, solve: day13::solve },
    Puzzle { generate: day14::generate, solve: day14::solve },
    Puzzle { generate: day15::generate, solve: day15::solve },
    Puzzle { generate: day16::generate, solve: day16::solve },
    Puzzle { generate: day17::generate, solve: day17::solve },
    Puzzle { generate: day18::generate, solve: day18::solve },
    Puzzle { generate: day19::generate, solve: day19::solve },
    Puzzle { generate: day20::generate, solve: day20::solve },
    Puzzle { generate: day21::generate, solve: day21::solve },
    Puzzle { generate: day22::generate, solve: day22::solve },
    Puzzle { generate: day23::generate, solve: day23::solve },
    Puzzle { generate: day24::generate, solve: day24::solve },
    Puzzle { generate: day25::generate, solve: day25::solve },
];

pub fn generate(user: &str, year: u32, day: u32) -> String {
    let mut rng = Rng::new(seed(user, year, day));
    (PUZZLES[(day - 1) as usize].generate)(&mut rng)
}

pub fn solve(user: &str, year: u32, day: u32) -> (String, String) {
    let input = generate(user, year, day);
    (PUZZLES[(day - 1) as usize].solve)(&input)
}
