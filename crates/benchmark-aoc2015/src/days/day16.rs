use crate::rng::Rng;

// The known facts from the problem:
//   children:3 cats:7 samoyeds:2 pomeranians:3 akitas:0 vizslas:0
//   goldfish:5 trees:3 cars:2 perfumes:1
// Part 1: find the Sue whose listed attributes all match exactly.
// Part 2: cats/trees are "greater than", pomeranians/goldfish "less than",
// others exact.
pub fn solve(input: &str) -> (String, String) {
    let facts = [
        ("children", 3i64),
        ("cats", 7),
        ("samoyeds", 2),
        ("pomeranians", 3),
        ("akitas", 0),
        ("vizslas", 0),
        ("goldfish", 5),
        ("trees", 3),
        ("cars", 2),
        ("perfumes", 1),
    ];
    let mut p1 = -1i64;
    let mut p2 = -1i64;
    for line in input.lines() {
        let l = line.trim();
        if l.is_empty() {
            continue;
        }
        // "Sue 1: children: 3, cats: 7, ..."
        let parts: Vec<&str> = l.split_whitespace().collect();
        let sue_num: i64 = parts[1].trim_end_matches(':').parse().unwrap();
        // gather attributes
        let mut attrs: Vec<(&str, i64)> = Vec::new();
        let mut i = 2;
        while i + 1 < parts.len() {
            let key = parts[i].trim_end_matches(':');
            let val: i64 = parts[i + 1].trim_end_matches(',').parse().unwrap();
            attrs.push((key, val));
            i += 2;
        }
        let mut ok1 = true;
        let mut ok2 = true;
        for (k, v) in &attrs {
            let fact = facts.iter().find(|(fk, _)| fk == k).unwrap().1;
            if v != &fact {
                ok1 = false;
            }
            match *k {
                "cats" | "trees" => {
                    if v <= &fact {
                        ok2 = false;
                    }
                }
                "pomeranians" | "goldfish" => {
                    if v >= &fact {
                        ok2 = false;
                    }
                }
                _ => {
                    if v != &fact {
                        ok2 = false;
                    }
                }
            }
        }
        if ok1 && p1 < 0 {
            p1 = sue_num;
        }
        if ok2 && p2 < 0 {
            p2 = sue_num;
        }
    }
    (p1.to_string(), p2.to_string())
}

pub fn generate(rng: &mut Rng) -> String {
    let facts = [
        ("children", 3i64),
        ("cats", 7),
        ("samoyeds", 2),
        ("pomeranians", 3),
        ("akitas", 0),
        ("vizslas", 0),
        ("goldfish", 5),
        ("trees", 3),
        ("cars", 2),
        ("perfumes", 1),
    ];
    let n = rng.range(100, 500) as usize;
    let mut out = String::new();
    // Sue matching part 1 (exact facts) and a (possibly same) Sue matching
    // part 2 (range rules). Both get 3 attributes equal to their target value.
    let p1_target = rng.below(n as u64) as usize;
    let p2_target = rng.below(n as u64) as usize;
    for i in 0..n {
        let mut chosen: Vec<usize> = Vec::new();
        let mut pool: Vec<usize> = (0..facts.len()).collect();
        for _ in 0..3 {
            let k = rng.below(pool.len() as u64) as usize;
            chosen.push(pool.remove(k));
        }
        let mut entries = Vec::new();
        for &ci in &chosen {
            let (name, fact) = facts[ci];
            let val = if i == p1_target {
                // exact match (satisfies part 1)
                fact
            } else if i == p2_target {
                // range match (satisfies part 2): cats/trees > fact,
                // pomeranians/goldfish < fact, others exact.
                match name {
                    "cats" | "trees" => fact + 1,
                    "pomeranians" | "goldfish" => fact - 1,
                    _ => fact,
                }
            } else {
                // Non-matching: force a violation that fails BOTH parts.
                match name {
                    "cats" | "trees" => rng.range(0, fact), // <= fact fails p2
                    "pomeranians" | "goldfish" => rng.range(fact + 1, fact + 5), // >= fact fails p2
                    _ => {
                        let mut v = rng.range(0, 10);
                        if v == fact {
                            v = fact + 1;
                        }
                        v
                    }
                }
            };
            entries.push(format!("{}: {}", name, val));
        }
        out.push_str(&format!("Sue {}: {}\n", i + 1, entries.join(", ")));
    }
    out
}
