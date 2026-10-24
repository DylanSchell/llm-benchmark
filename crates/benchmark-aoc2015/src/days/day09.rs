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
        let (a, b, d) = (
            parts[0].to_string(),
            parts[2].to_string(),
            parts[4].parse::<i64>().unwrap(),
        );
        cities.insert(a.clone());
        cities.insert(b.clone());
        dist.insert((a.clone(), b.clone()), d);
        dist.insert((b.clone(), a.clone()), d);
    }
    let list: Vec<String> = cities.into_iter().collect();
    let mut search = TourSearch {
        cities: &list,
        dist: &dist,
        used: vec![false; list.len()],
        shortest: i64::MAX,
        longest: i64::MIN,
    };
    search.run(0, 0, 0);
    (search.shortest.to_string(), search.longest.to_string())
}

/// Exhaustive permutation walk over the complete distance graph.
///
/// The graph, the visited flags and the running best/worst belong together: passing
/// them as eight positional arguments only moved the bookkeeping into the call site.
struct TourSearch<'a> {
    cities: &'a [String],
    dist: &'a HashMap<(String, String), i64>,
    used: Vec<bool>,
    shortest: i64,
    longest: i64,
}

impl TourSearch<'_> {
    /// Extend a partial route that has visited `count` cities and last stood on
    /// `last`, having accumulated `cur` distance so far.
    fn run(&mut self, count: usize, last: i64, cur: i64) {
        if count == self.cities.len() {
            self.shortest = self.shortest.min(cur);
            self.longest = self.longest.max(cur);
            return;
        }
        for i in 0..self.cities.len() {
            if self.used[i] {
                continue;
            }
            let d = if count == 0 {
                0
            } else {
                *self
                    .dist
                    .get(&(self.cities[last as usize].clone(), self.cities[i].clone()))
                    .unwrap()
            };
            self.used[i] = true;
            self.run(count + 1, i as i64, cur + d);
            self.used[i] = false;
        }
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
