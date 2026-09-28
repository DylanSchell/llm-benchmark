use crate::rng::Rng;

// Part 1: total code chars minus total in-memory chars.
// Part 2: total re-encoded chars minus total code chars.
pub fn solve(input: &str) -> (String, String) {
    let mut code = 0i64;
    let mut memory = 0i64;
    let mut reencoded = 0i64;
    for line in input.lines() {
        let s = line.trim();
        if s.is_empty() {
            continue;
        }
        code += s.len() as i64;
        memory += memory_len(s);
        reencoded += reencode_len(s);
    }
    ((code - memory).to_string(), (reencoded - code).to_string())
}

fn memory_len(s: &str) -> i64 {
    // s is quoted; content is between quotes.
    let inner = &s[1..s.len() - 1];
    let bytes = inner.as_bytes();
    let mut n = 0i64;
    let mut i = 0;
    while i < bytes.len() {
        if bytes[i] == b'\\' {
            // escape
            if i + 1 < bytes.len() && bytes[i + 1] == b'x' {
                i += 4; // \xNN
            } else {
                i += 2; // \\ or \"
            }
        } else {
            i += 1;
        }
        n += 1;
    }
    n
}

fn reencode_len(s: &str) -> i64 {
    let mut n = 2i64; // surrounding quotes
    for c in s.chars() {
        match c {
            '"' => n += 2, // \" becomes \\\"
            '\\' => n += 2,
            _ => n += 1,
        }
    }
    n
}

pub fn generate(rng: &mut Rng) -> String {
    let mut out = String::new();
    for _ in 0..300 {
        let len = rng.range(2, 14) as usize;
        let mut inner = String::new();
        for _ in 0..len {
            match rng.below(10) {
                0..=6 => inner.push(rng.lowercase()),
                7 => inner.push(rng.digit()),
                8 => inner.push_str("\\\\"),
                _ => inner.push('"'),
            }
        }
        out.push('"');
        out.push_str(&inner);
        out.push('"');
        out.push('\n');
    }
    out
}
