//! Deterministic PRNG seeded from (user, year, day).
//! Same seed -> same input, so `validate` can regenerate the input.

// SplitMix64 — small, fast, high-quality, deterministic across platforms.
pub struct Rng(u64);

impl Rng {
    pub fn new(seed: u64) -> Self {
        Rng(seed)
    }

    fn next_u64(&mut self) -> u64 {
        self.0 = self.0.wrapping_add(0x9E3779B97F4A7C15);
        let mut z = self.0;
        z = (z ^ (z >> 30)).wrapping_mul(0xBF58476D1CE4E5B9);
        z = (z ^ (z >> 27)).wrapping_mul(0x94D049BB133111EB);
        z ^ (z >> 31)
    }

    /// Uniform integer in [0, n). Panics if n == 0.
    pub fn below(&mut self, n: u64) -> u64 {
        assert!(n > 0);
        self.next_u64() % n
    }

    /// Uniform integer in [lo, hi] inclusive.
    pub fn range(&mut self, lo: i64, hi: i64) -> i64 {
        assert!(lo <= hi);
        lo + (self.below((hi - lo + 1) as u64) as i64)
    }

    /// Uniform bool.
    pub fn coin(&mut self) -> bool {
        self.below(2) == 1
    }

    /// Pick a random element from a slice.
    pub fn pick<'a, T>(&mut self, items: &'a [T]) -> &'a T {
        &items[self.below(items.len() as u64) as usize]
    }

    /// Random lowercase letter.
    pub fn lowercase(&mut self) -> char {
        (b'a' + self.below(26) as u8) as char
    }

    /// Random digit 0-9.
    pub fn digit(&mut self) -> char {
        (b'0' + self.below(10) as u8) as char
    }
}

/// Derive a u64 seed from (user, year, day).
pub fn seed(user: &str, year: u32, day: u32) -> u64 {
    // FNV-1a over the concatenation, then mix once more with SplitMix.
    let mut h: u64 = 0xcbf29ce484222325;
    for b in user.as_bytes() {
        h ^= *b as u64;
        h = h.wrapping_mul(0x100000001b3);
    }
    h ^= year as u64;
    h = h.wrapping_mul(0x100000001b3);
    h ^= day as u64;
    h = h.wrapping_mul(0x100000001b3);
    // Mix to decorrelate nearby seeds.
    let mut r = Rng::new(h);
    r.next_u64()
}
