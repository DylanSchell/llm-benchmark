use crate::rng::Rng;
use std::collections::{HashMap, HashSet};

// Part 1: shortest route visiting every city once.
// Part 2: longest such route.
pub fn solve(input: &str) -> (String, String) {
    let mut dist: HashMap<(String, String), i64> = HashMap::new();
    let mut cities = HashSet::new();
    for line in input.lines() {
        let l = line.trim();
        if l.is_empty() {
            continue;
        }
        let parts: Vec<&str> = l.split_whitespace().collect();
        let (a, b, d) = (parts[0].to_string(), parts[2].to_string(), parts[4].parse::<i64>().unwrap());
        cities.insert(a.clone());
        cities.insert(b.clone());
        dist.insert((a.clone(), b.clone()), d);
        dist.insert((b.clone(), a.clone()), d);
    }
    let list: Vec<String> = cities.into_iter().collect();
    let (mut shortest, mut longest) = (i64::MAX, i64::MIN);
    let mut used = vec![false; list.len()];
    dfs(&list, &dist, &mut used, 0, 0, 0, &mut shortest, &mut longest);
    (shortest.to_string(), longest.to_string())
}

fn dfs(
    cities: &[String],
    dist: &HashMap<(String, String), i64>,
    used: &mut [bool],
    count: usize,
    last: i64,
    cur: i64,
    shortest: &mut i64,
    longest: &mut i64,
) {
    if count == cities.len() {
        *shortest = (*shortest).min(cur);
        *longest = (*longest).max(cur);
        return;
    }
    for i in 0..cities.len() {
        if used[i] {
            continue;
        }
        let d = if count == 0 {
            0
        } else {
            *dist.get(&(cities[last as usize].clone(), cities[i].clone())).unwrap()
        };
        used[i] = true;
        dfs(cities, dist, used, count + 1, i as i64, cur + d, shortest, longest);
        used[i] = false;
    }
}

pub fn generate(rng: &mut Rng) -> String {
    // 8 cities, complete distance matrix.
    let n = rng.range(6, 9) as usize;
    let mut names = Vec::new();
    let mut seen = HashSet::new();
    while names.len() < n {
        let name = format!("C{}", rng.range(0, 1000));
        if seen.insert(name.clone()) {
            names.push(name);
        }
    }
    let mut out = String::new();
    for i in 0..n {
        for j in (i + 1)..n {
            let d = rng.range(10, 200);
            out.push_str(&format!("{} to {} = {}\n", names[i], names[j], d));
        }
    }
    out
}
