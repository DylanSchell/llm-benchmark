use crate::rng::Rng;

// Part 1: number of combinations of containers summing to 150.
// Part 2: number of ways using the fewest containers.
pub fn solve(input: &str) -> (String, String) {
    let target = 150i64;
    let caps: Vec<i64> = input.lines().filter_map(|l| l.trim().parse().ok()).collect();
    let n = caps.len();
    let mut count1 = 0i64;
    let mut min_containers = usize::MAX;
    let mut count_min = 0i64;
    for mask in 0u32..(1u32 << n) {
        let mut sum = 0i64;
        let mut cnt = 0usize;
        for i in 0..n {
            if mask & (1 << i) != 0 {
                sum += caps[i];
                cnt += 1;
            }
        }
        if sum == target {
            count1 += 1;
            if cnt < min_containers {
                min_containers = cnt;
                count_min = 1;
            } else if cnt == min_containers {
                count_min += 1;
            }
        }
    }
    (count1.to_string(), count_min.to_string())
}

pub fn generate(rng: &mut Rng) -> String {
    let n = rng.range(10, 20) as usize;
    let mut out = String::new();
    let mut vals: Vec<i64> = Vec::new();
    for _ in 0..n {
        vals.push(rng.range(10, 120));
    }
    // Guarantee at least one subset sums to 150 by appending a partition of 150
    // using a few of the values (or explicit filler if needed).
    // Simple approach: ensure the multiset contains a random subset summing to
    // exactly 150, then print all values.
    let mut remaining = 150i64;
    let mut extra: Vec<i64> = Vec::new();
    while remaining > 0 {
        let c = rng.range(1, remaining.min(120)).max(1);
        extra.push(c);
        remaining -= c;
    }
    vals.extend(extra);
    for v in vals {
        out.push_str(&format!("{}\n", v));
    }
    out
}
