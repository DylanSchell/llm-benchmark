use crate::rng::Rng;

// Part 1: number of lit lights after 100 steps of Conway's Game of Life.
// Part 2: same, but the four corners are always stuck on.
pub fn solve(input: &str) -> (String, String) {
    let grid = parse_grid(input);
    let mut g1 = grid.clone();
    for _ in 0..100 {
        g1 = step(&g1);
    }
    let p1 = count(&g1);

    let mut g2 = grid.clone();
    corners_on(&mut g2);
    for _ in 0..100 {
        g2 = step(&g2);
        corners_on(&mut g2);
    }
    let p2 = count(&g2);
    (p1.to_string(), p2.to_string())
}

fn parse_grid(input: &str) -> Vec<Vec<bool>> {
    input
        .lines()
        .filter(|l| !l.trim().is_empty())
        .map(|l| l.trim().chars().map(|c| c == '#').collect())
        .collect()
}

fn count(g: &[Vec<bool>]) -> i64 {
    g.iter().map(|r| r.iter().filter(|&&c| c).count() as i64).sum()
}

fn corners_on(g: &mut Vec<Vec<bool>>) {
    let n = g.len();
    if n == 0 {
        return;
    }
    let m = g[0].len();
    g[0][0] = true;
    g[0][m - 1] = true;
    g[n - 1][0] = true;
    g[n - 1][m - 1] = true;
}

fn step(g: &[Vec<bool>]) -> Vec<Vec<bool>> {
    let n = g.len();
    let m = g[0].len();
    let mut out = vec![vec![false; m]; n];
    for i in 0..n {
        for j in 0..m {
            let mut on = 0;
            for di in -1i64..=1 {
                for dj in -1i64..=1 {
                    if di == 0 && dj == 0 {
                        continue;
                    }
                    let ni = i as i64 + di;
                    let nj = j as i64 + dj;
                    if ni >= 0 && ni < n as i64 && nj >= 0 && nj < m as i64 {
                        if g[ni as usize][nj as usize] {
                            on += 1;
                        }
                    }
                }
            }
            out[i][j] = if g[i][j] { on == 2 || on == 3 } else { on == 3 };
        }
    }
    out
}

pub fn generate(rng: &mut Rng) -> String {
    // Use a moderately dense grid so some lights stay lit over 100 steps.
    let n = rng.range(12, 30) as usize;
    let m = n; // square
    let mut out = String::new();
    for _ in 0..n {
        for _ in 0..m {
            out.push(if rng.below(3) < 2 { '#' } else { '.' });
        }
        out.push('\n');
    }
    out
}
