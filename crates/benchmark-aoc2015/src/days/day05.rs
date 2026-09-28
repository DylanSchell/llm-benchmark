use crate::rng::Rng;

// Part 1 "nice": at least 3 vowels, one double letter, no ab/cd/pq/xy.
// Part 2 "nice": a pair of letters repeating non-overlapping, and a letter
// that repeats with exactly one letter between it.
pub fn solve(input: &str) -> (String, String) {
    let mut p1 = 0i64;
    let mut p2 = 0i64;
    for line in input.lines() {
        let s = line.trim();
        if nice1(s) {
            p1 += 1;
        }
        if nice2(s) {
            p2 += 1;
        }
    }
    (p1.to_string(), p2.to_string())
}

fn nice1(s: &str) -> bool {
    let chars: Vec<char> = s.chars().collect();
    let vowels = "aeiou";
    let vowel_count = chars.iter().filter(|c| vowels.contains(**c)).count();
    if vowel_count < 3 {
        return false;
    }
    let mut double = false;
    for w in chars.windows(2) {
        if w[0] == w[1] {
            double = true;
            break;
        }
    }
    if !double {
        return false;
    }
    !["ab", "cd", "pq", "xy"].iter().any(|b| s.contains(b))
}

fn nice2(s: &str) -> bool {
    let chars: Vec<char> = s.chars().collect();
    // pair of letters repeating with no overlap
    let mut pair = false;
    'outer: for i in 0..chars.len().saturating_sub(1) {
        for j in (i + 2)..chars.len().saturating_sub(1) {
            if chars[i] == chars[j] && chars[i + 1] == chars[j + 1] {
                pair = true;
                break 'outer;
            }
        }
    }
    if !pair {
        return false;
    }
    // letter repeating with one between
    for i in 0..chars.len().saturating_sub(2) {
        if chars[i] == chars[i + 2] {
            return true;
        }
    }
    false
}

pub fn generate(rng: &mut Rng) -> String {
    let mut s = String::new();
    for _ in 0..1000 {
        let len = rng.range(6, 18) as usize;
        for _ in 0..len {
            s.push(rng.lowercase());
        }
        s.push('\n');
    }
    s
}
