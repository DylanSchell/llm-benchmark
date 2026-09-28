use crate::rng::Rng;
use std::collections::{HashMap, HashSet};

// Part 1: max happiness in a circular seating of all guests.
// Part 2: add "Me" (0 happiness with everyone), re-optimize.
pub fn solve(input: &str) -> (String, String) {
    let mut pairs: HashMap<(String, String), i64> = HashMap::new();
    let mut people = HashSet::new();
    for line in input.lines() {
        let l = line.trim();
        if l.is_empty() {
            continue;
        }
        let parts: Vec<&str> = l.split_whitespace().collect();
        // "Alice would gain 2 happiness units by sitting next to Bob."
        let a = parts[0].to_string();
        let sign = if parts[2] == "gain" { 1 } else { -1 };
        let v: i64 = parts[3].parse().unwrap();
        let b = parts[10].trim_end_matches('.').to_string();
        people.insert(a.clone());
        people.insert(b.clone());
        pairs.insert((a, b), sign * v);
    }
    let list: Vec<String> = people.into_iter().collect();
    let p1 = best(&list, &pairs);

    // Part 2: add "Me".
    let mut list2 = list.clone();
    list2.push("Me".to_string());
    let mut pairs2 = pairs.clone();
    for p in &list {
        pairs2.insert((p.clone(), "Me".to_string()), 0);
        pairs2.insert(("Me".to_string(), p.clone()), 0);
    }
    let p2 = best(&list2, &pairs2);
    (p1.to_string(), p2.to_string())
}

fn best(list: &[String], pairs: &HashMap<(String, String), i64>) -> i64 {
    let n = list.len();
    let mut used = vec![false; n];
    let mut best = i64::MIN;
    let mut order: Vec<usize> = Vec::new();
    dfs(list, pairs, &mut used, &mut order, &mut best);
    best
}

fn dfs(list: &[String], pairs: &HashMap<(String, String), i64>, used: &mut [bool], order: &mut Vec<usize>, best: &mut i64) {
    if order.len() == list.len() {
        let mut total = 0i64;
        for i in 0..order.len() {
            let a = &list[order[i]];
            let b = &list[order[(i + 1) % order.len()]];
            total += pairs[&(a.clone(), b.clone())];
            total += pairs[&(b.clone(), a.clone())];
        }
        *best = (*best).max(total);
        return;
    }
    for i in 0..list.len() {
        if used[i] {
            continue;
        }
        used[i] = true;
        order.push(i);
        dfs(list, pairs, used, order, best);
        order.pop();
        used[i] = false;
    }
}

pub fn generate(rng: &mut Rng) -> String {
    let n = rng.range(6, 9) as usize;
    let mut names = Vec::new();
    let mut seen = HashSet::new();
    while names.len() < n {
        let name = format!("Guest{}", rng.range(0, 1000));
        if seen.insert(name.clone()) {
            names.push(name);
        }
    }
    let mut out = String::new();
    for a in &names {
        for b in &names {
            if a == b {
                continue;
            }
            let v = rng.range(1, 100);
            let gain = rng.coin();
            let word = if gain { "gain" } else { "lose" };
            out.push_str(&format!(
                "{} would {} {} happiness units by sitting next to {}.\n",
                a, word, v, b
            ));
        }
    }
    out
}
