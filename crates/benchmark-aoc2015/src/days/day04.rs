use crate::md5::md5_hex;
use crate::rng::Rng;

// Find the lowest positive integer whose MD5(prefix+num) hex starts with
// 5 (part1) or 6 (part2) zeroes.
pub fn solve(input: &str) -> (String, String) {
    let prefix = input.trim();
    let mut part1 = 0i64;
    let mut part2 = 0i64;
    let mut n = 1i64;
    loop {
        let h = md5_hex(format!("{}{}", prefix, n).as_bytes());
        if part1 == 0 && h.starts_with("00000") {
            part1 = n;
        }
        if part2 == 0 && h.starts_with("000000") {
            part2 = n;
        }
        if part1 != 0 && part2 != 0 {
            break;
        }
        n += 1;
    }
    (part1.to_string(), part2.to_string())
}

pub fn generate(rng: &mut Rng) -> String {
    // A random 8-char lowercase alphanumeric prefix.
    let mut s = String::new();
    for _ in 0..8 {
        s.push(match rng.below(36) {
            n if n < 26 => (b'a' + n as u8) as char,
            n => (b'0' + (n - 26) as u8) as char,
        });
    }
    s.push('\n');
    s
}
