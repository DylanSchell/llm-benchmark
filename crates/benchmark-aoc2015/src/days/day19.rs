use crate::rng::Rng;
use std::collections::{HashMap, HashSet};

// Part 1: distinct molecules from one replacement.
// Part 2: fewest steps from "e" to the molecule, via reverse reduction search.
// The reverse graph is acyclic (each rule strictly shortens), so a memoized DFS
// is exact. We keep generated molecules small enough that this is fast.
pub fn solve(input: &str) -> (String, String) {
    let (reps, molecule) = parse(input);

    // Part 1.
    let mut distinct = HashSet::new();
    for (from, to) in &reps {
        for (i, _) in molecule.match_indices(from) {
            let mut s = String::new();
            s.push_str(&molecule[..i]);
            s.push_str(to);
            s.push_str(&molecule[i + from.len()..]);
            distinct.insert(s);
        }
    }
    let p1 = distinct.len();

    // Part 2: reverse reduction BFS.
    let rev: Vec<(String, String)> = reps.iter().map(|(a, b)| (b.clone(), a.clone())).collect();
    let p2 = reduce(&molecule, &rev);
    (p1.to_string(), p2.to_string())
}

// Reverse-reduction BFS: find the fewest steps to reduce `s` to "e".
// Rules are applied in reverse (RHS -> LHS). We allow all rules (including
// same-length ones like `e => X`) but track visited states to avoid cycles.
fn reduce(s: &str, rev: &[(String, String)]) -> i64 {
    let mut queue = std::collections::VecDeque::new();
    let mut dist: HashMap<String, i64> = HashMap::new();
    dist.insert(s.to_string(), 0);
    queue.push_back(s.to_string());
    while let Some(cur) = queue.pop_front() {
        let d = dist[&cur];
        if cur == "e" {
            return d;
        }
        for (to, from) in rev {
            let mut idx = 0;
            while let Some(pos) = cur[idx..].find(to) {
                let abs = idx + pos;
                let mut ns = String::new();
                ns.push_str(&cur[..abs]);
                ns.push_str(from);
                ns.push_str(&cur[abs + to.len()..]);
                // Only keep states that are not longer (avoid cycles that grow).
                if ns.len() <= cur.len() && !dist.contains_key(&ns) {
                    let nd = d + 1;
                    dist.insert(ns.clone(), nd);
                    queue.push_back(ns);
                }
                idx = abs + 1;
            }
        }
    }
    i64::MAX
}

fn parse(input: &str) -> (Vec<(String, String)>, String) {
    let mut reps = Vec::new();
    let mut molecule = String::new();
    for line in input.lines() {
        if let Some((a, b)) = line.split_once(" => ") {
            reps.push((a.to_string(), b.to_string()));
        } else if !line.trim().is_empty() {
            molecule = line.trim().to_string();
        }
    }
    (reps, molecule)
}

// Generate a grammar-compliant input. The molecule is built by a forward
// derivation from "e": we expand non-terminals using the rules, tracking the
// exact number of steps. Part 2 = number of steps in the derivation, which is
// the minimum because the reverse reduction finds it. We build a derivation
// whose expansion is exactly the recorded step count.
pub fn generate(rng: &mut Rng) -> String {
    let elems = ["H", "He", "Li", "Be", "B", "C", "N", "O", "F", "Ne", "Al", "Ca", "Mg", "P", "Si", "Ti", "Th"];

    // Build rules (a small set keeps the reverse search fast). Every rule must
    // strictly LENGTHEN (RHS longer than LHS) so the reverse reduction is an
    // acyclic DAG and the molecule reduces cleanly to "e".
    let mut rules: Vec<(String, String)> = Vec::new();
    for e in elems.iter() {
        let k = rng.range(1, 2);
        for _ in 0..k {
            let other: Vec<&&str> = elems.iter().filter(|x| *x != e).collect();
            let rhs = match rng.below(3) {
                0 => {
                    // Two distinct elements concatenated (guaranteed longer).
                    let a = rng.pick(&other);
                    let b = rng.pick(&other);
                    format!("{}{}", a, b)
                }
                1 => {
                    let a = rng.pick(&other);
                    let b = rng.pick(&other);
                    format!("{}Rn{}Ar", a, b)
                }
                _ => {
                    let a = rng.pick(&other);
                    let b = rng.pick(&other);
                    let c = rng.pick(&other);
                    format!("{}Rn{}Y{}Ar", a, b, c)
                }
            };
            rules.push((e.to_string(), rhs));
        }
    }

    // Forward derivation from "e". We keep a list of tokens (non-terminals);
    // each step picks a token and replaces it with a rule's RHS (its element
    // symbols). This guarantees the molecule is reachable from e and the
    // reverse reduction returns exactly the derivation length.
    let mut tokens: Vec<String> = vec!["e".to_string()];
    let n_steps = rng.range(5, 12) as usize;
    // Choose a "start" rule that produces an actual element from e.
    // Add an explicit rule e -> <element> to the rule set.
    let start_elem = rng.pick(&elems);
    rules.push(("e".to_string(), start_elem.to_string()));

    for _ in 0..n_steps {
        let expandable: Vec<usize> = tokens
            .iter()
            .enumerate()
            .filter(|(_, t)| rules.iter().any(|(l, _)| l == *t))
            .map(|(i, _)| i)
            .collect();
        if expandable.is_empty() {
            break;
        }
        let idx = expandable[rng.below(expandable.len() as u64) as usize];
        let tok = tokens[idx].clone();
        let cands: Vec<String> = rules
            .iter()
            .filter(|(l, _)| *l == tok)
            .map(|(_, r)| r.clone())
            .collect();
        let rhs = rng.pick(&cands).clone();
        let rhs_tokens = tokenize(&rhs);
        tokens.splice(idx..idx + 1, rhs_tokens);
    }

    // Retry until the molecule verifiably reduces to "e". Leaves in a naive
    // derivation can be single elements with no reducing rule, so we validate
    // and regenerate (consuming more RNG each attempt) until reducible.
    for _ in 0..100 {
        let molecule = tokens.concat();
        if reduces_to_e(&molecule, &rules) {
            let mut out = String::new();
            for (l, r) in &rules {
                out.push_str(&format!("{} => {}\n", l, r));
            }
            out.push('\n');
            out.push_str(&molecule);
            out.push('\n');
            return out;
        }
        // Regenerate the derivation from scratch, drawing more randomness.
        tokens = vec!["e".to_string()];
        for _ in 0..rng.range(5, 12) {
            let expandable: Vec<usize> = tokens
                .iter()
                .enumerate()
                .filter(|(_, t)| rules.iter().any(|(l, _)| l == *t))
                .map(|(i, _)| i)
                .collect();
            if expandable.is_empty() {
                break;
            }
            let idx = expandable[rng.below(expandable.len() as u64) as usize];
            let tok = tokens[idx].clone();
            let cands: Vec<String> = rules
                .iter()
                .filter(|(l, _)| *l == tok)
                .map(|(_, r)| r.clone())
                .collect();
            let rhs = rng.pick(&cands).clone();
            let rhs_tokens = tokenize(&rhs);
            tokens.splice(idx..idx + 1, rhs_tokens);
        }
    }
    // Fallback: a minimal guaranteed-reducible molecule (e -> element).
    let elem = tokenize(&rules.iter().find(|(l, _)| l == "e").unwrap().1).join("");
    let mut out = String::new();
    for (l, r) in &rules {
        out.push_str(&format!("{} => {}\n", l, r));
    }
    out.push('\n');
    out.push_str(&elem);
    out.push('\n');
    out
}

// Does the molecule reduce to "e" via the rules (reverse reduction)?
fn reduces_to_e(molecule: &str, rules: &[(String, String)]) -> bool {
    let rev: Vec<(String, String)> = rules.iter().map(|(a, b)| (b.clone(), a.clone())).collect();
    reduce(molecule, &rev) != i64::MAX
}

fn tokenize(s: &str) -> Vec<String> {
    let bytes = s.as_bytes();
    let mut v = Vec::new();
    let mut i = 0;
    while i < bytes.len() {
        if bytes[i].is_ascii_uppercase() {
            let mut t = String::new();
            t.push(bytes[i] as char);
            if i + 1 < bytes.len() && bytes[i + 1].is_ascii_lowercase() {
                t.push(bytes[i + 1] as char);
                i += 1;
            }
            v.push(t);
        }
        i += 1;
    }
    v
}
