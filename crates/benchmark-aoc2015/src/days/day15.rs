use crate::rng::Rng;

// Part 1: max cookie score using exactly 100 tsp (each ingredient 0..=100).
// Score = product of positive sums of capacity*count, durability*count, etc.
// Part 2: same but require exactly 500 calories.
pub fn solve(input: &str) -> (String, String) {
    let mut ing = Vec::new();
    for line in input.lines() {
        let l = line.trim();
        if l.is_empty() {
            continue;
        }
        let nums: Vec<i64> = l
            .split([' ', ','])
            .filter_map(|t| t.parse().ok())
            .collect();
        // capacity, durability, flavor, texture, calories
        ing.push((nums[0], nums[1], nums[2], nums[3], nums[4]));
    }
    let n = ing.len();
    let mut best1 = 0i64;
    let mut best2 = 0i64;
    let mut counts = vec![0i64; n];
    // iterate compositions of 100 over n ingredients
    fn rec(
        ing: &[(i64, i64, i64, i64, i64)],
        idx: usize,
        remaining: i64,
        counts: &mut [i64],
        best1: &mut i64,
        best2: &mut i64,
    ) {
        if idx == counts.len() - 1 {
            counts[idx] = remaining;
            let mut cap = 0i64;
            let mut dur = 0i64;
            let mut fla = 0i64;
            let mut tex = 0i64;
            let mut cal = 0i64;
            for (k, &c) in counts.iter().enumerate() {
                cap += c * ing[k].0;
                dur += c * ing[k].1;
                fla += c * ing[k].2;
                tex += c * ing[k].3;
                cal += c * ing[k].4;
            }
            let score = cap.max(0) * dur.max(0) * fla.max(0) * tex.max(0);
            *best1 = (*best1).max(score);
            if cal == 500 {
                *best2 = (*best2).max(score);
            }
            return;
        }
        for c in 0..=remaining {
            counts[idx] = c;
            rec(ing, idx + 1, remaining - c, counts, best1, best2);
        }
    }
    rec(&ing, 0, 100, &mut counts, &mut best1, &mut best2);
    (best1.to_string(), best2.to_string())
}

pub fn generate(rng: &mut Rng) -> String {
    let n = rng.range(4, 6) as usize;
    let mut out = String::new();
    // Ensure at least one ingredient has positive attributes (so part 1 > 0)
    // and that the calorie range allows reaching exactly 500.
    for i in 0..n {
        let mut cap = rng.range(-8, 8);
        let mut dur = rng.range(-8, 8);
        let mut fla = rng.range(-8, 8);
        let mut tex = rng.range(-8, 8);
        if i == 0 {
            cap = cap.max(2);
            dur = dur.max(2);
            fla = fla.max(2);
            tex = tex.max(2);
        }
        // calorie values that allow a total of exactly 500 with 100 tsp.
        let cal = rng.range(3, 12);
        out.push_str(&format!(
            "Ingredient{}: capacity {}, durability {}, flavor {}, texture {}, calories {}\n",
            i, cap, dur, fla, tex, cal
        ));
    }
    out
}
