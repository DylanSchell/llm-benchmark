use crate::rng::Rng;

// Part 1: wrapping paper = 2*lw + 2*wh + 2*lh + slack (min face area).
// Part 2: ribbon = 2*(sum of two smallest) + volume.
pub fn solve(input: &str) -> (String, String) {
    let mut paper = 0i64;
    let mut ribbon = 0i64;
    for line in input.lines() {
        let dims: Vec<i64> =
            line.trim().split('x').map(|x| x.parse().unwrap()).collect();
        let (l, w, h) = (dims[0], dims[1], dims[2]);
        let (a, b, c) = (l * w, w * h, h * l);
        paper += 2 * (a + b + c) + a.min(b).min(c);
        let mut d = [l, w, h];
        d.sort();
        ribbon += 2 * (d[0] + d[1]) + d[0] * d[1] * d[2];
    }
    (paper.to_string(), ribbon.to_string())
}

pub fn generate(rng: &mut Rng) -> String {
    let mut s = String::new();
    for _ in 0..1000 {
        let l = rng.range(1, 30);
        let w = rng.range(1, 30);
        let h = rng.range(1, 30);
        s.push_str(&format!("{}x{}x{}\n", l, w, h));
    }
    s
}
