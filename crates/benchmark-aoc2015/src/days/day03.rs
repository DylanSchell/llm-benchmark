use crate::rng::Rng;
use std::collections::HashSet;

// Part 1: unique houses visited by Santa.
// Part 2: Santa and Robo-Santa alternate moves; unique houses visited by both.
pub fn solve(input: &str) -> (String, String) {
    let dirs: Vec<(i32, i32)> = input
        .trim()
        .chars()
        .map(|c| match c {
            '^' => (0, 1),
            'v' => (0, -1),
            '>' => (1, 0),
            '<' => (-1, 0),
            _ => (0, 0),
        })
        .collect();

    let mut visited = HashSet::new();
    let (mut x, mut y) = (0i32, 0i32);
    visited.insert((x, y));
    for &(dx, dy) in &dirs {
        x += dx;
        y += dy;
        visited.insert((x, y));
    }
    let part1 = visited.len() as i64;

    let mut visited2 = HashSet::new();
    let (mut sx, mut sy) = (0i32, 0i32);
    let (mut rx, mut ry) = (0i32, 0i32);
    visited2.insert((0, 0));
    for (i, &(dx, dy)) in dirs.iter().enumerate() {
        if i % 2 == 0 {
            sx += dx;
            sy += dy;
            visited2.insert((sx, sy));
        } else {
            rx += dx;
            ry += dy;
            visited2.insert((rx, ry));
        }
    }
    (part1.to_string(), (visited2.len() as i64).to_string())
}

pub fn generate(rng: &mut Rng) -> String {
    let n = rng.range(3000, 9000) as usize;
    let mut s = String::with_capacity(n);
    for _ in 0..n {
        s.push(match rng.below(4) {
            0 => '^',
            1 => 'v',
            2 => '>',
            _ => '<',
        });
    }
    s.push('\n');
    s
}
