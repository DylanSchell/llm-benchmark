use crate::rng::Rng;

// Part 1: floor = +1 for '(' , -1 for ')'.
// Part 2: first position (1-indexed) where the running floor goes negative.
pub fn solve(input: &str) -> (String, String) {
    let mut floor = 0i64;
    let mut part2 = -1i64;
    for (i, c) in input.trim().chars().enumerate() {
        floor += if c == '(' { 1 } else { -1 };
        if part2 < 0 && floor < 0 {
            part2 = (i + 1) as i64;
        }
    }
    (floor.to_string(), part2.to_string())
}

pub fn generate(rng: &mut Rng) -> String {
    let n = rng.range(4000, 8000) as usize;
    let mut s = String::with_capacity(n);
    for _ in 0..n {
        s.push(if rng.coin() { '(' } else { ')' });
    }
    s.push('\n');
    s
}
