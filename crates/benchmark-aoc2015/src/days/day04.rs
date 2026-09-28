use md5::{Digest, Md5};
use crate::rng::Rng;

// Find the lowest positive integer whose MD5(prefix+num) hex starts with
// 5 (part1) or 6 (part2) zeroes.
//
// Uses RustCrypto's `md-5` crate. The candidate is written into a reused buffer
// (no per-iteration String allocation) and the raw digest bytes are checked for
// the zero-prefix instead of formatting a hex string.
pub fn solve(input: &str) -> (String, String) {
    let prefix = input.trim();
    let mut part1 = 0i64;
    let mut part2 = 0i64;
    let mut n = 1i64;
    // Reused buffer for prefix + n. The prefix is ASCII; n is appended as a
    // decimal string. Capacity is generous to avoid reallocation.
    let mut buf = Vec::with_capacity(prefix.len() + 16);
    buf.extend_from_slice(prefix.as_bytes());
    let base_len = buf.len();
    loop {
        // Write n into the buffer after the prefix (no heap alloc per iteration).
        buf.truncate(base_len);
        let mut tmp = [0u8; 20];
        let mut len = 0;
        let mut v = n;
        if v == 0 {
            tmp[0] = b'0';
            len = 1;
        } else {
            while v > 0 {
                tmp[len] = b'0' + (v % 10) as u8;
                v /= 10;
                len += 1;
            }
            tmp[..len].reverse();
        }
        buf.extend_from_slice(&tmp[..len]);

        let h = Md5::digest(&buf);
        // Check leading hex zeroes directly on the 16-byte digest.
        // 5 zeroes -> first 2 bytes zero and high nibble of byte 2 zero.
        // 6 zeroes -> first 3 bytes zero.
        if part1 == 0 && h[0] == 0 && h[1] == 0 && (h[2] & 0xF0) == 0 {
            part1 = n;
        }
        if part2 == 0 && h[0] == 0 && h[1] == 0 && h[2] == 0 {
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

#[cfg(test)]
mod tests {
    use super::*;

    // The AoC example input `abcdef` must produce the documented answers, and
    // part 2 (the ~16.7M-op search) must finish quickly. This guards both
    // correctness and the allocation-free optimisation.
    #[test]
    fn abcdef_produces_known_answers_fast() {
        let t = std::time::Instant::now();
        let (p1, p2) = solve("abcdef");
        let el = t.elapsed();
        assert_eq!(p1, "609043");
        assert_eq!(p2, "6742839");
        // Release-mode solves part 2 in well under 2s. Debug builds are slower
        // (the ~16.7M-op MD5 loop), so the bound is generous; the point is it
        // must not hang for minutes as the un-optimised version did.
        assert!(el.as_secs() < 30, "day 4 solve too slow: {:?}", el);
    }

    // A 5-zero answer must be found before the 6-zero one, and both positive.
    #[test]
    fn answers_are_ordered() {
        let (p1, p2) = solve("iwrupvqb");
        let a = p1.parse::<i64>().unwrap();
        let b = p2.parse::<i64>().unwrap();
        assert!(a > 0 && b > 0);
        assert!(a < b);
    }
}
