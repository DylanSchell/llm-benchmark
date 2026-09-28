use crate::rng::Rng;
use std::collections::HashMap;

// Part 1: evaluate circuit, value of wire `a`.
// Part 2: override `b` with the value of `a`, re-evaluate, value of `a`.
pub fn solve(input: &str) -> (String, String) {
    let mut ops: Vec<(String, String)> = Vec::new();
    for line in input.lines() {
        let l = line.trim();
        if l.is_empty() {
            continue;
        }
        let (lhs, rhs) = l.split_once("->").unwrap();
        ops.push((lhs.trim().to_string(), rhs.trim().to_string()));
    }

    let p1 = eval(&ops, "a", &mut HashMap::new());
    // Part 2: set b = a's value, recompute a.
    let mut ops2 = ops.clone();
    for (lhs, rhs) in ops2.iter_mut() {
        if rhs == "b" {
            *lhs = p1.to_string();
        }
    }
    let p2 = eval(&ops2, "a", &mut HashMap::new());
    (p1.to_string(), p2.to_string())
}

fn eval(ops: &[(String, String)], wire: &str, memo: &mut HashMap<String, i64>) -> i64 {
    if let Some(v) = memo.get(wire) {
        return *v;
    }
    // Find the definition.
    let def = ops.iter().find(|(_, rhs)| rhs == wire);
    let (lhs, _) = def.expect("wire not defined");
    let toks: Vec<&str> = lhs.split_whitespace().collect();
    let val = if toks.len() == 1 {
        parse_value(ops, toks[0], memo)
    } else if toks[0] == "NOT" {
        !parse_value(ops, toks[1], memo) & 0xffff
    } else {
        let a = parse_value(ops, toks[0], memo);
        let b = parse_value(ops, toks[2], memo);
        match toks[1] {
            "AND" => a & b,
            "OR" => a | b,
            "LSHIFT" => (a << b) & 0xffff,
            "RSHIFT" => a >> b,
            _ => unreachable!(),
        }
    };
    memo.insert(wire.to_string(), val);
    val
}

fn parse_value(ops: &[(String, String)], tok: &str, memo: &mut HashMap<String, i64>) -> i64 {
    if let Ok(n) = tok.parse::<i64>() {
        n & 0xffff
    } else {
        eval(ops, tok, memo)
    }
}

pub fn generate(rng: &mut Rng) -> String {
    // Build a topological chain b -> w1 -> w2 -> ... -> a, so that `a` depends
    // on `b`. Part 2 overrides `b` with a's value, which changes a's result.
    let chain_len = rng.range(8, 20) as usize;
    let mut wires: Vec<String> = vec!["b".to_string()];
    for i in 1..chain_len {
        wires.push(format!("w{}", i));
    }
    wires.push("a".to_string());

    let mut out = String::new();
    // Define b from a constant.
    out.push_str(&format!("{} -> b\n", rng.range(0, 65535)));
    // Build a chain that keeps `a` genuinely sensitive to `b`. Use only bijective
    // ops (NOT and 1-bit shifts) so the composed function is a bijection: it's
    // never constant, and f(f(x)) != f(x) generically. AND/OR masks are avoided
    // because they collapse the 16-bit space.
    for i in 1..wires.len() {
        let target = &wires[i];
        let prev = &wires[i - 1];
        let def = match rng.below(3) {
            0 => format!("NOT {} -> {}", prev, target),
            1 => format!("{} LSHIFT 1 -> {}", prev, target),
            _ => format!("{} RSHIFT 1 -> {}", prev, target),
        };
        out.push_str(&def);
        out.push('\n');
    }
    out
}
