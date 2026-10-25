use crate::rng::Rng;
use std::cmp::Ordering;
use std::collections::BinaryHeap;

const SPELLS: [(i64, i64, i64, i64, usize); 5] = [
    (53, 4, 0, 0, 3),  // Magic Missile
    (73, 2, 2, 0, 3),  // Drain
    (113, 0, 0, 6, 0), // Shield (armor 7)
    (173, 0, 0, 6, 1), // Poison (3 dmg/turn)
    (229, 0, 0, 5, 2), // Recharge (101 mana/turn)
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

#[derive(Clone, Copy, PartialEq, Eq)]
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

/// Everything about a fight that decides how it can continue, minus the mana spent so
/// far. `bdmg` is fixed for a whole search, so it is left out.
type Position = (i64, i64, i64, i64, i64, i64);

fn position(s: &State) -> Position {
    (s.hp, s.mana, s.bhp, s.shield, s.poison, s.recharge)
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
    // Effects resolve at the start of the player's turn, and the first character at or
    // below 0 hit points loses. A boss killed by Poison *there* is dead before another
    // spell is cast, so the fight ends with no further mana spent — including when the
    // player is too broke to cast anything at all. The returned cost is 0: no spell.
    if cur.bhp <= 0 {
        return Some((cur, 0, true));
    }
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
    cur = apply_effects(&cur);
    if cur.bhp <= 0 {
        return Some((cur, cost, true));
    }
    // Armour is whatever Shield is active *after* the boss-turn tick: an effect whose
    // timer reaches 0 has ended, so a shield expiring here blocks nothing.
    let armor = if cur.shield > 0 { 7 } else { 0 };
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
    // Dijkstra-style memo: the cheapest cost at which we've already expanded a
    // position. Reaching the same position at a higher (or equal) cost can never lead
    // to a cheaper win, so it can be pruned. Without this, the search re-expands
    // identical positions many times and explodes for high-HP / low-damage bosses.
    //
    // Keyed on the position *without* `spent`: `spent` is what we are minimising, so
    // keying on it would make every path look like a new position and prune nothing.
    let mut best_seen: std::collections::HashMap<Position, i64> = std::collections::HashMap::new();
    best_seen.insert(position(&start), 0);
    while let Some(s) = heap.pop() {
        // Skip if a cheaper path already expanded this position.
        if best_seen.get(&position(&s)).copied().unwrap_or(i64::MAX) < s.spent {
            continue;
        }
        if s.spent >= best {
            continue;
        }
        for spell in 0..SPELLS.len() {
            if let Some((next, _, won)) = simulate(&s, spell, hard) {
                if won {
                    best = best.min(next.spent);
                } else if next.spent < best {
                    // Only expand if this is the cheapest known cost for `next`.
                    match best_seen.entry(position(&next)) {
                        std::collections::hash_map::Entry::Occupied(mut e) => {
                            if next.spent < *e.get() {
                                e.insert(next.spent);
                                heap.push(next);
                            }
                        }
                        std::collections::hash_map::Entry::Vacant(e) => {
                            e.insert(next.spent);
                            heap.push(next);
                        }
                    }
                }
            }
        }
    }
    best
}

pub fn generate(rng: &mut Rng) -> String {
    // The search space for (hp, dmg) includes combos with no winning strategy
    // (e.g. high boss damage with moderate HP), for which `search` returns
    // i64::MAX and the puzzle is genuinely unsolvable. The generator therefore
    // samples and validates: if a candidate can't be won, it resamples. The
    // search is fast (memoised), so this adds negligible latency.
    for _ in 0..64 {
        let hp = rng.range(30, 80);
        let dmg = rng.range(5, 12);
        let input = format!("Hit Points: {hp}\nDamage: {dmg}\n");
        let (p1, p2) = solve(&input);
        if p1 != i64::MAX.to_string() && p2 != i64::MAX.to_string() {
            return input;
        }
    }
    // Fallback: a known-solvable, fast input (matches the real AoC puzzle).
    "Hit Points: 71\nDamage: 10\n".to_string()
}

#[cfg(test)]
mod gen_tests {
    use super::*;
    use crate::rng::Rng;

    // The generator must never emit an unsolvable input. Sample many seeds and
    // assert every generated input solves to a finite value for both parts.
    #[test]
    fn generator_never_emits_unsolvable_input() {
        const MAX: &str = "9223372036854775807";
        for i in 0..200u64 {
            // Build a deterministic Rng without the seed(user,year,day) helper.
            let mut rng = Rng::new(i.wrapping_mul(0x9E3779B97F4A7C15).wrapping_add(0x1234567));
            let input = generate(&mut rng);
            let (p1, p2) = solve(&input);
            assert_ne!(p1, MAX, "generated unsolvable part 1: {input}");
            assert_ne!(p2, MAX, "generated unsolvable part 2: {input}");
        }
    }

    // The rules as written in `descriptions/day22.md`, applied to the real Advent of
    // Code 2015 input (also the generator's fallback). Unchanged by the turn-start fix
    // below — that is why this input never exposed the bug.
    #[test]
    fn solves_the_real_2015_input() {
        let (p1, p2) = solve("Hit Points: 71\nDamage: 10\n");
        assert_eq!((p1.as_str(), p2.as_str()), ("1824", "1937"));
    }

    // Effects resolve at the start of the player's turn, so Poison that brings the boss
    // to 0 there wins the fight before another spell is cast or paid for. The solver
    // used to fall through to the cast, overcharging by a spell (and treating a broke
    // player as having lost a fight they had already won).
    #[test]
    fn poison_kill_at_turn_start_costs_no_extra_spell() {
        let s = State {
            hp: 20,
            mana: 0,
            bhp: 2,
            bdmg: 10,
            shield: 0,
            poison: 1,
            recharge: 0,
            spent: 1824,
        };
        let (next, _cost, won) =
            simulate(&s, 0, false).expect("poison kills the boss at the turn start");
        assert!(won);
        assert_eq!(
            next.spent, 1824,
            "no mana may be charged after the boss dies"
        );
    }

    // An effect whose timer reaches 0 has ended, so a shield expiring at the start of
    // the boss turn blocks nothing.
    #[test]
    fn shield_expiring_on_the_boss_turn_blocks_nothing() {
        let s = State {
            hp: 50,
            mana: 500,
            bhp: 100,
            bdmg: 10,
            shield: 2,
            poison: 0,
            recharge: 0,
            spent: 0,
        };
        let (next, _, won) = simulate(&s, 0, false).expect("both survive the round");
        assert!(!won);
        assert_eq!(next.shield, 0);
        assert_eq!(next.hp, 40, "an expired shield must not reduce boss damage");
    }

    // The second worked example in the puzzle text, round by round: player 10 hp /
    // 250 mana, boss 14 hp / 8 damage, casting Recharge, Shield, Drain, Poison, Magic
    // Missile. Every value asserted here is quoted from `descriptions/day22.md`, so
    // this is the authoritative check on what a "turn" means.
    #[test]
    fn reproduces_the_puzzle_text_walkthrough() {
        let mut s = State {
            hp: 10,
            mana: 250,
            bhp: 14,
            bdmg: 8,
            shield: 0,
            poison: 0,
            recharge: 0,
            spent: 0,
        };
        let casts = [4usize, 2, 1, 3, 0];
        // (hp, mana, boss hp, shield, poison, recharge) as the text logs them at the
        // start of the next player turn.
        let after = [
            (2, 122, 14, 0, 0, 4),
            (1, 211, 14, 5, 0, 2),
            (2, 340, 12, 3, 0, 0),
            (1, 167, 9, 1, 5, 0),
        ];
        for (i, (hp, mana, bhp, shield, poison, recharge)) in after.into_iter().enumerate() {
            let (next, _, won) = simulate(&s, casts[i], false).expect("player survives the round");
            assert!(!won, "round {} should not end the fight", i + 1);
            assert_eq!(
                (
                    next.hp,
                    next.mana,
                    next.bhp,
                    next.shield,
                    next.poison,
                    next.recharge
                ),
                (hp, mana, bhp, shield, poison, recharge),
                "after round {}",
                i + 1
            );
            s = next;
        }
        // "Poison deals 3 damage. This kills the boss, and the player wins."
        let (next, _, won) = simulate(&s, casts[4], false).expect("player wins");
        assert!(won);
        assert_eq!(next.spent, 641);
    }

    // The default seed in the web UI generates a 79 hp / 7 damage boss, whose part 2 is
    // won by Poison at the start of a player turn. The old solver charged one more Magic
    // Missile (1877), so a correct agent could never match it.
    #[test]
    fn default_seed_day22_is_scored_at_the_true_optimum() {
        let input = crate::days::generate("benchmark", 2015, 22);
        assert_eq!(input, "Hit Points: 79\nDamage: 7\n");
        let (p1, p2) = solve(&input);
        assert_eq!((p1.as_str(), p2.as_str()), ("1764", "1824"));
    }
}
