use crate::rng::Rng;

// Part 1: lowest house with >= target presents (10 * sum-of-divisors).
// Part 2: each elf visits at most 50 houses, delivers 11 * elf.
pub fn solve(input: &str) -> (String, String) {
    let target: i64 = input.trim().parse().unwrap();
    let p1 = find_house(target, 10, i64::MAX);
    let p2 = find_house(target, 11, 50);
    (p1.to_string(), p2.to_string())
}

fn find_house(target: i64, per_elf: i64, max_visits: i64) -> i64 {
    let limit = (target / per_elf).max(1) + 1;
    let mut presents = vec![0i64; limit as usize];
    for elf in 1..limit {
        let mut house = elf;
        let mut visits = 0;
        while house < limit && visits < max_visits {
            presents[house as usize] += elf * per_elf;
            house += elf;
            visits += 1;
        }
    }
    for (i, &p) in presents.iter().enumerate() {
        if p >= target {
            return i as i64;
        }
    }
    -1
}

pub fn generate(rng: &mut Rng) -> String {
    // Larger targets make the 50-visit cap bind, so parts 1 and 2 differ.
    format!("{}\n", rng.range(5_000_000, 40_000_000))
}
