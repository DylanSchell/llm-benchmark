use crate::rng::Rng;

#[derive(Clone, Debug)]
enum Instr {
    Hlf(char),
    Tpl(char),
    Inc(char),
    Jmp(i64),
    Jie(char, i64),
    Jio(char, i64),
}

// Part 1: run with a=0, return b. Part 2: run with a=1, return b.
pub fn solve(input: &str) -> (String, String) {
    let prog = parse(input);
    let (_, b1) = run(&prog, 0, 0);
    let (_, b2) = run(&prog, 1, 0);
    (b1.to_string(), b2.to_string())
}

fn parse(input: &str) -> Vec<Instr> {
    let mut prog = Vec::new();
    for line in input.lines() {
        let t = line.trim();
        if t.is_empty() {
            continue;
        }
        let parts: Vec<&str> = t.split_whitespace().collect();
        match parts[0] {
            "hlf" => prog.push(Instr::Hlf(parts[1].chars().next().unwrap())),
            "tpl" => prog.push(Instr::Tpl(parts[1].chars().next().unwrap())),
            "inc" => prog.push(Instr::Inc(parts[1].chars().next().unwrap())),
            "jmp" => prog.push(Instr::Jmp(parts[1].parse().unwrap())),
            "jie" => {
                let r = parts[1].chars().next().unwrap();
                let off: i64 = parts[2].trim_start_matches('+').parse().unwrap();
                prog.push(Instr::Jie(r, off));
            }
            "jio" => {
                let r = parts[1].chars().next().unwrap();
                let off: i64 = parts[2].trim_start_matches('+').parse().unwrap();
                prog.push(Instr::Jio(r, off));
            }
            _ => {}
        }
    }
    prog
}

fn run(prog: &[Instr], a: i64, b: i64) -> (i64, i64) {
    let mut a = a;
    let mut b = b;
    let mut pc: i64 = 0;
    while pc >= 0 && (pc as usize) < prog.len() {
        match &prog[pc as usize] {
            Instr::Hlf(r) => {
                if *r == 'a' { a /= 2; } else { b /= 2; }
                pc += 1;
            }
            Instr::Tpl(r) => {
                if *r == 'a' { a *= 3; } else { b *= 3; }
                pc += 1;
            }
            Instr::Inc(r) => {
                if *r == 'a' { a += 1; } else { b += 1; }
                pc += 1;
            }
            Instr::Jmp(off) => pc += off,
            Instr::Jie(r, off) => {
                let v = if *r == 'a' { a } else { b };
                pc += if v % 2 == 0 { *off } else { 1 };
            }
            Instr::Jio(r, off) => {
                let v = if *r == 'a' { a } else { b };
                pc += if v == 1 { *off } else { 1 };
            }
        }
    }
    (a, b)
}

pub fn generate(rng: &mut Rng) -> String {
    // Build a program that always halts and yields a non-zero `b`, while making
    // part 1 (a=0) and part 2 (a=1) differ. Structure:
    //   - a short prologue that manipulates `a` and branches on it,
    //   - then a deterministic loop that increments `b`.
    let loop_count = rng.range(3, 12) as i64;
    let mut prog: Vec<Instr> = Vec::new();
    // Prologue: make `a` depend on its initial value so part1/part2 diverge.
    prog.push(Instr::Inc('a'));
    prog.push(Instr::Tpl('a'));
    // Conditional: if a is odd/even, do an extra step so the two runs differ.
    // We use jie/jio with a forward skip.
    let skip = rng.range(2, 4) as i64;
    prog.push(Instr::Jie('a', skip)); // if a even, skip `skip` forward
    // (when a starts 1: after inc,tpl -> a=3 odd, so no jump; when a starts 0:
    //  inc,tpl -> a=2 even, jump forward)
    prog.push(Instr::Inc('b')); // extra b when odd path
    prog.push(Instr::Inc('a'));
    prog.push(Instr::Inc('a'));
    // Loop: repeat `loop_count` times, incrementing b.
    // Use register a as a counter.
    // We'll set a = loop_count by a short sequence, then loop.
    // To keep it simple and terminating, generate a straight-line sequence of
    // inc b repeated, plus a few hlf/tpl on a for variety.
    for _ in 0..loop_count {
        prog.push(Instr::Inc('b'));
        match rng.below(3) {
            0 => prog.push(Instr::Inc('a')),
            1 => prog.push(Instr::Tpl('a')),
            _ => prog.push(Instr::Hlf('a')),
        }
    }
    let mut out = String::new();
    for instr in prog {
        out.push_str(&format!("{}\n", fmt(&instr)));
    }
    out
}

fn fmt(i: &Instr) -> String {
    match i {
        Instr::Hlf(r) => format!("hlf {}", r),
        Instr::Tpl(r) => format!("tpl {}", r),
        Instr::Inc(r) => format!("inc {}", r),
        Instr::Jmp(o) => format!("jmp +{}", o),
        Instr::Jie(r, o) => format!("jie {}, +{}", r, o),
        Instr::Jio(r, o) => format!("jio {}, +{}", r, o),
    }
}
