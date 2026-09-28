use crate::rng::Rng;

// Part 1: partition into 3 equal groups; fewest packages in group 1, then
// minimal quantum entanglement (product).
// Part 2: partition into 4 equal groups.
pub fn solve(input: &str) -> (String, String) {
    let weights = parse(input);
    let p1 = best_first_group(&weights, 3);
    let p2 = best_first_group(&weights, 4);
    (p1.to_string(), p2.to_string())
}

fn parse(input: &str) -> Vec<i64> {
    input.lines().filter_map(|l| l.trim().parse().ok()).collect()
}

fn best_first_group(weights: &[i64], groups: usize) -> i128 {
    let total: i64 = weights.iter().sum();
    if total % groups as i64 != 0 {
        // Shouldn't happen for generated input; fall back to a large value.
        return -1;
    }
    let target = total / groups as i64;
    let mut best_size = usize::MAX;
    let mut best_qe = i128::MAX;
    let mut subset = Vec::new();
    search(weights, 0, target, &mut subset, groups, &mut best_size, &mut best_qe);
    best_qe
}

fn search(
    weights: &[i64],
    idx: usize,
    remaining: i64,
    subset: &mut Vec<usize>,
    groups: usize,
    best_size: &mut usize,
    best_qe: &mut i128,
) {
    if subset.len() > *best_size {
        return;
    }
    if remaining == 0 {
        let size = subset.len();
        if size > *best_size {
            return;
        }
        if !can_partition(weights, subset, groups) {
            return;
        }
        let qe: i128 = subset.iter().map(|&i| weights[i] as i128).product();
        if size < *best_size || (size == *best_size && qe < *best_qe) {
            *best_size = size;
            *best_qe = qe;
        }
        return;
    }
    if idx >= weights.len() || remaining < 0 {
        return;
    }
    subset.push(idx);
    search(weights, idx + 1, remaining - weights[idx], subset, groups, best_size, best_qe);
    subset.pop();
    search(weights, idx + 1, remaining, subset, groups, best_size, best_qe);
}

fn can_partition(weights: &[i64], chosen: &[usize], groups: usize) -> bool {
    let total: i64 = weights.iter().sum::<i64>() / groups as i64;
    let mut rest: Vec<i64> = Vec::new();
    let mut chosen_set = vec![false; weights.len()];
    for &i in chosen {
        chosen_set[i] = true;
    }
    for (i, &w) in weights.iter().enumerate() {
        if !chosen_set[i] {
            rest.push(w);
        }
    }
    rest.sort_by(|a, b| b.cmp(a));
    let mut buckets = vec![0i64; groups - 1];
    can_fill(&rest, 0, total, &mut buckets)
}

fn can_fill(items: &[i64], idx: usize, target: i64, buckets: &mut [i64]) -> bool {
    if idx == items.len() {
        return buckets.iter().all(|&b| b == target);
    }
    let w = items[idx];
    for i in 0..buckets.len() {
        if buckets[i] + w <= target {
            buckets[i] += w;
            if can_fill(items, idx + 1, target, buckets) {
                return true;
            }
            buckets[i] -= w;
        }
        if buckets[i] == 0 {
            break;
        }
    }
    false
}

pub fn generate(rng: &mut Rng) -> String {
    // Generate weights that partition evenly into both 3 and 4 groups.
    // Build 3 groups summing to X each, then split into 4 requires total
    // divisible by 4 too. Choose a base that's divisible by both 3 and 4 (12),
    // then create a multiset whose total is divisible by 12.
    let base = rng.range(1, 8) as i64; // group weight unit
    let unit = base * 12; // divisible by 3 and 4
    // Number of "units" spread across weights. We create random weights that
    // sum to a multiple of 12.
    let n = rng.range(12, 28) as usize;
    let mut weights: Vec<i64> = (0..n).map(|_| rng.range(1, unit)).collect();
    // Adjust so total is divisible by 12.
    let total: i64 = weights.iter().sum();
    let rem = total % 12;
    if rem != 0 {
        let i = rng.below(n as u64) as usize;
        weights[i] += (12 - rem) % 12;
    }
    let mut out = String::new();
    for w in weights {
        out.push_str(&format!("{}\n", w));
    }
    out
}
