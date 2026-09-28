use crate::json::{Json, parse};
use crate::rng::Rng;

// Part 1: sum all numbers. Part 2: sum all numbers, ignoring any object whose
// values include the string "red" (recursively).
pub fn solve(input: &str) -> (String, String) {
    let root = parse(input).expect("valid json");
    let p1 = sum_all(&root);
    let p2 = sum_no_red(&root);
    (p1.to_string(), p2.to_string())
}

fn sum_all(v: &Json) -> i64 {
    match v {
        Json::Num(n) => *n,
        Json::Array(a) => a.iter().map(sum_all).sum(),
        Json::Object(o) => o.iter().map(|(_, x)| sum_all(x)).sum(),
        _ => 0,
    }
}

fn sum_no_red(v: &Json) -> i64 {
    match v {
        Json::Num(n) => *n,
        Json::Array(a) => a.iter().map(sum_no_red).sum(),
        Json::Object(o) => {
            if o.iter().any(|(_, x)| matches!(x, Json::Str(s) if s == "red")) {
                0
            } else {
                o.iter().map(|(_, x)| sum_no_red(x)).sum()
            }
        }
        _ => 0,
    }
}

pub fn generate(rng: &mut Rng) -> String {
    // Top-level array so part 2 always sums something (objects can be red).
    let n = rng.range(3, 8) as usize;
    let items: Vec<String> = (0..n).map(|_| gen_value(rng, 0)).collect();
    let mut out = format!("[{}]", items.join(","));
    out.push('\n');
    out
}

fn gen_value(rng: &mut Rng, depth: usize) -> String {
    if depth > 4 {
        return gen_scalar(rng);
    }
    match rng.below(4) {
        0 => gen_scalar(rng),
        1 => gen_array(rng, depth),
        _ => gen_object(rng, depth),
    }
}

fn gen_scalar(rng: &mut Rng) -> String {
    match rng.below(4) {
        0 => format!("{}", rng.range(-100, 100)),
        1 => format!("\"{}\"", rng.lowercase()),
        2 => "true".to_string(),
        _ => "null".to_string(),
    }
}

fn gen_array(rng: &mut Rng, depth: usize) -> String {
    let n = rng.range(1, 6) as usize;
    let items: Vec<String> = (0..n).map(|_| gen_value(rng, depth + 1)).collect();
    format!("[{}]", items.join(","))
}

fn gen_object(rng: &mut Rng, depth: usize) -> String {
    let n = rng.range(1, 6) as usize;
    let mut items = Vec::new();
    for _ in 0..n {
        // Sometimes include a "red" value to exercise part 2 (not too often).
        let key = format!("\"{}\"", rng.lowercase());
        let val = if rng.below(5) == 0 {
            "\"red\"".to_string()
        } else {
            gen_value(rng, depth + 1)
        };
        items.push(format!("{}:{}", key, val));
    }
    format!("{{{}}}", items.join(","))
}
