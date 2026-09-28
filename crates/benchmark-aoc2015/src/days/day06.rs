use crate::rng::Rng;

// Part 1: 1000x1000 lights, turn on/off/toggle, count lit.
// Part 2: brightness, turn on +1, off -1 (min 0), toggle +2, sum.
pub fn solve(input: &str) -> (String, String) {
    let mut grid1 = vec![false; 1000 * 1000];
    let mut grid2 = vec![0i64; 1000 * 1000];
    for line in input.lines() {
        let l = line.trim();
        let (op, rest) = if let Some(r) = l.strip_prefix("turn on ") {
            (0u8, r)
        } else if let Some(r) = l.strip_prefix("turn off ") {
            (1u8, r)
        } else if let Some(r) = l.strip_prefix("toggle ") {
            (2u8, r)
        } else {
            continue;
        };
        let nums: Vec<i64> = rest
            .split([' ', ','])
            .filter_map(|t| t.parse().ok())
            .collect();
        let (x1, y1, x2, y2) = (nums[0], nums[1], nums[2], nums[3]);
        for x in x1..=x2 {
            for y in y1..=y2 {
                let idx = (x * 1000 + y) as usize;
                match op {
                    0 => {
                        grid1[idx] = true;
                        grid2[idx] += 1;
                    }
                    1 => {
                        grid1[idx] = false;
                        grid2[idx] = (grid2[idx] - 1).max(0);
                    }
                    _ => {
                        grid1[idx] = !grid1[idx];
                        grid2[idx] += 2;
                    }
                }
            }
        }
    }
    let p1 = grid1.iter().filter(|&&v| v).count() as i64;
    let p2: i64 = grid2.iter().sum();
    (p1.to_string(), p2.to_string())
}

pub fn generate(rng: &mut Rng) -> String {
    let mut s = String::new();
    for _ in 0..300 {
        let op = rng.below(3);
        let (x1, x2) = sorted_pair(rng);
        let (y1, y2) = sorted_pair(rng);
        let cmd = match op {
            0 => "turn on ",
            1 => "turn off ",
            _ => "toggle ",
        };
        s.push_str(&format!("{}{},{} through {},{}\n", cmd, x1, y1, x2, y2));
    }
    s
}

fn sorted_pair(rng: &mut Rng) -> (i64, i64) {
    let a = rng.range(0, 999);
    let b = rng.range(0, 999);
    if a <= b { (a, b) } else { (b, a) }
}
