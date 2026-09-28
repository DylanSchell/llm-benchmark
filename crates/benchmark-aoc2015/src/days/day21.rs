use crate::rng::Rng;

const WEAPONS: [Item; 5] = [
    Item { cost: 8, damage: 4, armor: 0 },
    Item { cost: 10, damage: 5, armor: 0 },
    Item { cost: 25, damage: 6, armor: 0 },
    Item { cost: 40, damage: 7, armor: 0 },
    Item { cost: 74, damage: 8, armor: 0 },
];
const ARMORS: [Item; 5] = [
    Item { cost: 13, damage: 0, armor: 1 },
    Item { cost: 31, damage: 0, armor: 2 },
    Item { cost: 53, damage: 0, armor: 3 },
    Item { cost: 75, damage: 0, armor: 4 },
    Item { cost: 102, damage: 0, armor: 5 },
];
const RINGS: [Item; 6] = [
    Item { cost: 25, damage: 1, armor: 0 },
    Item { cost: 50, damage: 2, armor: 0 },
    Item { cost: 100, damage: 3, armor: 0 },
    Item { cost: 20, damage: 0, armor: 1 },
    Item { cost: 40, damage: 0, armor: 2 },
    Item { cost: 80, damage: 0, armor: 3 },
];

#[derive(Clone, Copy)]
struct Item {
    cost: i64,
    damage: i64,
    armor: i64,
}

struct Boss {
    hp: i64,
    damage: i64,
    armor: i64,
}

// Part 1: least gold to win. Part 2: most gold while still losing.
pub fn solve(input: &str) -> (String, String) {
    let boss = parse(input);
    let (mut min_win, mut max_lose) = (i64::MAX, i64::MIN);
    each_loadout(&mut |cost, dmg, armor| {
        let win = player_wins(&boss, dmg, armor);
        if win && cost < min_win {
            min_win = cost;
        }
        if !win && cost > max_lose {
            max_lose = cost;
        }
    });
    (min_win.to_string(), max_lose.to_string())
}

fn parse(input: &str) -> Boss {
    let mut hp = 0;
    let mut damage = 0;
    let mut armor = 0;
    for line in input.lines() {
        if let Some(v) = line.strip_prefix("Hit Points:") {
            hp = v.trim().parse().unwrap();
        } else if let Some(v) = line.strip_prefix("Damage:") {
            damage = v.trim().parse().unwrap();
        } else if let Some(v) = line.strip_prefix("Armor:") {
            armor = v.trim().parse().unwrap();
        }
    }
    Boss { hp, damage, armor }
}

fn player_wins(boss: &Boss, dmg: i64, armor: i64) -> bool {
    let player_hp = 100i64;
    let to_boss = (dmg - boss.armor).max(1);
    let to_player = (boss.damage - armor).max(1);
    let player_hits = (boss.hp + to_boss - 1) / to_boss;
    let boss_hits = (player_hp + to_player - 1) / to_player;
    player_hits <= boss_hits
}

fn each_loadout(f: &mut dyn FnMut(i64, i64, i64)) {
    for w in 0..WEAPONS.len() {
        for a in 0..=ARMORS.len() {
            for r1 in 0..=RINGS.len() {
                for r2 in (r1 + 1)..=RINGS.len() {
                    let mut dmg = WEAPONS[w].damage;
                    let mut armor = 0;
                    let mut cost = WEAPONS[w].cost;
                    if a < ARMORS.len() {
                        dmg += ARMORS[a].damage;
                        armor += ARMORS[a].armor;
                        cost += ARMORS[a].cost;
                    }
                    if r1 < RINGS.len() {
                        dmg += RINGS[r1].damage;
                        armor += RINGS[r1].armor;
                        cost += RINGS[r1].cost;
                    }
                    if r2 < RINGS.len() {
                        dmg += RINGS[r2].damage;
                        armor += RINGS[r2].armor;
                        cost += RINGS[r2].cost;
                    }
                    f(cost, dmg, armor);
                }
            }
        }
    }
}

pub fn generate(rng: &mut Rng) -> String {
    let hp = rng.range(50, 150);
    let dmg = rng.range(5, 15);
    let armor = rng.range(0, 8);
    format!("Hit Points: {}\nDamage: {}\nArmor: {}\n", hp, dmg, armor)
}
