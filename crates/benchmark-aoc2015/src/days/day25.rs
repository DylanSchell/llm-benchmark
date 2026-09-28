use crate::rng::Rng;

const MOD: i64 = 33554393;
const MULT: i64 = 252533;
const FIRST: i64 = 20151125;

// Part 1: the code at (row, col). Part 2: day 25 has no second part.
pub fn solve(input: &str) -> (String, String) {
    let (row, col) = parse(input);
    let idx = index(row, col);
    let mut code = FIRST;
    for _ in 1..idx {
        code = (code * MULT) % MOD;
    }
    (code.to_string(), String::new())
}

fn parse(input: &str) -> (i64, i64) {
    let nums: Vec<i64> = input
        .split_whitespace()
        .filter_map(|t| {
            let s: String = t.trim_matches(|c: char| !c.is_ascii_digit()).to_string();
            s.parse().ok()
        })
        .collect();
    (nums[0], nums[1])
}

fn index(row: i64, col: i64) -> i64 {
    let d = row + col - 1;
    d * (d - 1) / 2 + col
}

pub fn generate(rng: &mut Rng) -> String {
    let row = rng.range(1, 5000);
    let col = rng.range(1, 5000);
    format!("To continue, please consult the code grid in the manual.  Enter the code at row {}, column {}.\n", row, col)
}
