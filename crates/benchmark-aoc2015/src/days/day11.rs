use crate::rng::Rng;

// Part 1: next valid password. Part 2: the one after that.
// Rules: 8 lowercase, increasing straight of 3, no i/o/l, two non-overlapping pairs.
pub fn solve(input: &str) -> (String, String) {
    let p = input.trim().to_string();
    let first = next_valid(&p);
    let second = next_valid(&first);
    (first, second)
}

fn next_valid(p: &str) -> String {
    let mut bytes: Vec<u8> = p.bytes().collect();
    loop {
        increment(&mut bytes);
        if valid(&bytes) {
            return bytes.iter().map(|&b| b as char).collect();
        }
    }
}

fn increment(bytes: &mut [u8]) {
    let mut i = bytes.len();
    loop {
        i -= 1;
        bytes[i] += 1;
        if bytes[i] <= b'z' {
            break;
        }
        bytes[i] = b'a';
    }
}

fn valid(bytes: &[u8]) -> bool {
    // no i/o/l
    if bytes.iter().any(|&b| b == b'i' || b == b'o' || b == b'l') {
        return false;
    }
    // increasing straight of 3
    let mut straight = false;
    for w in bytes.windows(3) {
        if w[0] + 1 == w[1] && w[1] + 1 == w[2] {
            straight = true;
            break;
        }
    }
    if !straight {
        return false;
    }
    // two non-overlapping pairs
    let mut pairs = 0;
    let mut i = 0;
    while i + 1 < bytes.len() {
        if bytes[i] == bytes[i + 1] {
            pairs += 1;
            i += 2;
        } else {
            i += 1;
        }
    }
    pairs >= 2
}

pub fn generate(rng: &mut Rng) -> String {
    // 8 random lowercase letters.
    let mut s = String::new();
    for _ in 0..8 {
        s.push(rng.lowercase());
    }
    s.push('\n');
    s
}
