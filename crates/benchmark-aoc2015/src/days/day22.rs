use crate::rng::Rng;
use std::cmp::Ordering;
use std::collections::BinaryHeap;

const SPELLS: [(i64, i64, i64, i64, usize); 5] = [
    (53, 4, 0, 0, 3),   // Magic Missile
    (73, 2, 2, 0, 3),   // Drain
    (113, 0, 0, 6, 0),  // Shield (armor 7)
    (173, 0, 0, 6, 1),  // Poison (3 dmg/turn)
    (229, 0, 0, 5, 2),  // Recharge (101 mana/turn)
];

const PLAYER_HP: i64 = 50;
const PLAYER_MANA: i64 = 500;

// Part 1: min mana to win. Part 2: hard mode (lose 1 hp each player turn).
pub fn solve(input: &str) -> (String, String) {
    let (hp, dmg) = parse(input);
    let p1 = search(hp, dmg, false);
    let p2 = search(hp, dmg, true);
    (p1.to_string(), p2.to_string())
}

fn parse(input: &str) -> (i64, i64) {
    let mut hp = 0;
    let mut dmg = 0;
    for line in input.lines() {
        if let Some(v) = line.strip_prefix("Hit Points:") {
            hp = v.trim().parse().unwrap();
        } else if let Some(v) = line.strip_prefix("Damage:") {
            dmg = v.trim().parse().unwrap();
        }
    }
    (hp, dmg)
}

#[derive(Clone, Copy, PartialEq, Eq, Hash)]
struct State {
    hp: i64,
    mana: i64,
    bhp: i64,
    bdmg: i64,
    shield: i64,
    poison: i64,
    recharge: i64,
    spent: i64,
}

impl Ord for State {
    fn cmp(&self, other: &Self) -> Ordering {
        other.spent.cmp(&self.spent)
    }
}
impl PartialOrd for State {
    fn partial_cmp(&self, other: &Self) -> Option<Ordering> {
        Some(self.cmp(other))
    }
}

fn apply_effects(s: &State) -> State {
    let mut cur = *s;
    if cur.poison > 0 {
        cur.bhp -= 3;
        cur.poison -= 1;
    }
    if cur.recharge > 0 {
        cur.mana += 101;
        cur.recharge -= 1;
    }
    if cur.shield > 0 {
        cur.shield -= 1;
    }
    cur
}

fn simulate(s: &State, spell: usize, hard: bool) -> Option<(State, i64, bool)> {
    let mut cur = *s;
    if hard {
        cur.hp -= 1;
        if cur.hp <= 0 {
            return None;
        }
    }
    cur = apply_effects(&cur);
    let (cost, dmg, heal, dur, effect) = SPELLS[spell];
    if cur.mana < cost {
        return None;
    }
    cur.mana -= cost;
    cur.spent += cost;
    cur.bhp -= dmg;
    cur.hp += heal;
    if effect != 3 {
        let timer = match effect {
            0 => cur.shield,
            1 => cur.poison,
            _ => cur.recharge,
        };
        if timer > 0 {
            return None;
        }
        match effect {
            0 => cur.shield = dur,
            1 => cur.poison = dur,
            _ => cur.recharge = dur,
        }
    }
    if cur.bhp <= 0 {
        return Some((cur, cost, true));
    }
    let shield_active = cur.shield > 0;
    cur = apply_effects(&cur);
    if cur.bhp <= 0 {
        return Some((cur, cost, true));
    }
    let armor = if shield_active { 7 } else { 0 };
    let dmg_to_player = (cur.bdmg - armor).max(1);
    cur.hp -= dmg_to_player;
    if cur.hp <= 0 {
        return None;
    }
    Some((cur, cost, false))
}

fn search(bhp0: i64, bdmg: i64, hard: bool) -> i64 {
    let start = State {
        hp: PLAYER_HP,
        mana: PLAYER_MANA,
        bhp: bhp0,
        bdmg,
        shield: 0,
        poison: 0,
        recharge: 0,
        spent: 0,
    };
    let mut heap = BinaryHeap::new();
    heap.push(start);
    let mut best = i64::MAX;
    while let Some(s) = heap.pop() {
        if s.spent >= best {
            continue;
        }
        for spell in 0..SPELLS.len() {
            if let Some((next, _, won)) = simulate(&s, spell, hard) {
                if won {
                    best = best.min(next.spent);
                } else if next.spent < best {
                    heap.push(next);
                }
            }
        }
    }
    best
}

pub fn generate(rng: &mut Rng) -> String {
    let hp = rng.range(30, 80);
    let dmg = rng.range(5, 12);
    format!("Hit Points: {}\nDamage: {}\n", hp, dmg)
}
