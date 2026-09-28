use crate::rng::Rng;

// Look-and-say: length after 40 (part1) and 50 (part2) iterations.
pub fn solve(input: &str) -> (String, String) {
    let mut s = input.trim().to_string();
    let mut p1 = 0i64;
    for i in 1..=50 {
        s = look_and_say(&s);
        if i == 40 {
            p1 = s.len() as i64;
        }
    }
    (p1.to_string(), (s.len() as i64).to_string())
}

fn look_and_say(s: &str) -> String {
    let bytes = s.as_bytes();
    let mut out = String::new();
    let mut i = 0;
    while i < bytes.len() {
        let mut j = i;
        while j < bytes.len() && bytes[j] == bytes[i] {
            j += 1;
        }
        out.push_str(&(j - i).to_string());
        out.push(bytes[i] as char);
        i = j;
    }
    out
}

pub fn generate(rng: &mut Rng) -> String {
    // A short seed of digits.
    let len = rng.range(3, 8) as usize;
    let mut s = String::new();
    for _ in 0..len {
        s.push(rng.digit());
    }
    s.push('\n');
    s
}
