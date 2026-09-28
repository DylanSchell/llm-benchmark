use crate::rng::Rng;

// Part 1: distance traveled after 2503 seconds (best reindeer).
// Part 2: points awarded each second to the leader.
pub fn solve(input: &str) -> (String, String) {
    const T: i64 = 2503;
    let mut reindeer = Vec::new();
    for line in input.lines() {
        let l = line.trim();
        if l.is_empty() {
            continue;
        }
        let parts: Vec<&str> = l.split_whitespace().collect();
        let speed: i64 = parts[3].parse().unwrap();
        let run: i64 = parts[6].parse().unwrap();
        let rest: i64 = parts[13].parse().unwrap();
        reindeer.push((speed, run, rest));
    }
    let mut dists = Vec::new();
    let mut points = vec![0i64; reindeer.len()];
    for t in 1..=T {
        let mut best_dist = 0i64;
        let mut best_idx = Vec::new();
        for (i, &(speed, run, rest)) in reindeer.iter().enumerate() {
            let cycle = run + rest;
            let full = t / cycle;
            let rem = (t % cycle).min(run);
            let d = full * run * speed + rem * speed;
            if dists.len() <= i {
                dists.push(0);
            }
            dists[i] = d;
            if d > best_dist {
                best_dist = d;
                best_idx = vec![i];
            } else if d == best_dist {
                best_idx.push(i);
            }
        }
        for &i in &best_idx {
            points[i] += 1;
        }
    }
    let p1 = dists.iter().max().copied().unwrap();
    let p2 = points.iter().max().copied().unwrap();
    (p1.to_string(), p2.to_string())
}

pub fn generate(rng: &mut Rng) -> String {
    let n = rng.range(6, 12) as usize;
    let mut out = String::new();
    for i in 0..n {
        let speed = rng.range(5, 30);
        let run = rng.range(2, 20);
        let rest = rng.range(10, 200);
        out.push_str(&format!(
            "Deer{} can fly {} km/s for {} seconds, but then must rest for {} seconds.\n",
            i, speed, run, rest
        ));
    }
    out
}
