//! 打地鼠机制单测：锁的是**满盘闸**与排程形状，不是某张表。

use rand::rngs::StdRng;
use rand::SeedableRng;

use super::mole::{
    build_schedule, in_window, settle_score, validate_mole, BOMB_SPAWNS,
    DEFAULT_BOMB_PENALTY, FIRST_SPAWN_MS, HIT_EARLY_MS, HIT_LATE_MS,
    POOL_SPAWNS, SPAWN_GAP_MS, UP_MS,
};
use super::{EntryKind, PoolEntry};

const TICKET: i64 = 100;

fn magic(label: &str, weight: u32, mult: i64) -> PoolEntry {
    PoolEntry {
        label: label.to_string(),
        weight,
        kind: EntryKind::Magic {
            mult_permille: mult,
        },
    }
}

/// 与 0511 播种同形：单次 EV 0.077，满盘 0.924
fn seeded() -> Vec<PoolEntry> {
    vec![
        magic("谢谢参与", 590, 0),
        magic("灰鼠", 200, 50),
        magic("蓝耳鼠", 100, 100),
        magic("粉猪鼠", 55, 200),
        magic("白兔鼠", 30, 500),
        magic("墨镜酷鼠", 20, 800),
        magic("金鼠大奖", 5, 3_000),
    ]
}

#[test]
fn seeded_pool_passes_full_board_gate() {
    assert!(validate_mole(&seeded(), TICKET).is_ok());
    // 满盘 = 单次 EV × 12 次冒头，必须 < 0.95
    let ev = super::pool_ev(&seeded(), TICKET);
    assert!((ev * POOL_SPAWNS as f64) < 0.95, "ev={ev}");
}

#[test]
fn rich_pool_is_rejected_even_below_one_per_spawn() {
    // 单次 0.5 的池子「单抽 EV < 1」，但一局冒 12 次 → 6 倍票价
    let rich = vec![magic("谢谢参与", 500, 0), magic("半价大奖", 500, 500)];
    assert!(validate_mole(&rich, TICKET).is_err());
    assert!(super::validate_pool(&rich, TICKET).is_ok());
}

#[test]
fn item_tier_is_rejected() {
    let mut p = seeded();
    p.push(PoolEntry {
        label: "限定道具".into(),
        weight: 10,
        kind: EntryKind::Item {
            item_key: "k".into(),
            qty: 1,
            anchor: 10,
        },
    });
    assert!(validate_mole(&p, TICKET).is_err());
}

#[test]
fn schedule_shape_is_playable() {
    let mut rng = StdRng::seed_from_u64(7);
    let pool = seeded();
    let rar = [1, 1, 2, 3, 4, 4, 5];
    let s = build_schedule(&pool, TICKET, 25, &rar, &mut rng);
    assert_eq!(s.len(), POOL_SPAWNS + BOMB_SPAWNS);
    let bombs = s.iter().filter(|x| x.idx < 0).count();
    assert_eq!(bombs, BOMB_SPAWNS);
    for (i, x) in s.iter().enumerate() {
        assert_eq!(x.seq, i as i32);
        assert_eq!(x.at, FIRST_SPAWN_MS + i as i64 * SPAWN_GAP_MS);
        assert_eq!(x.up, UP_MS);
        assert!(x.hole < 9);
        if i > 0 {
            assert_ne!(x.hole, s[i - 1].hole, "同一洞连冒两次");
            assert!(x.at > s[i - 1].at);
        }
        if x.idx < 0 {
            assert_eq!(x.value, -25);
        } else {
            assert_eq!(x.rarity, rar[x.idx as usize]);
            assert!(x.value >= 0);
        }
    }
}

#[test]
fn window_has_grace_on_both_sides() {
    let mut rng = StdRng::seed_from_u64(9);
    let s = build_schedule(&seeded(), TICKET, 25, &[1; 7], &mut rng);
    let x = &s[3];
    assert!(in_window(x, x.at - HIT_EARLY_MS));
    assert!(!in_window(x, x.at - HIT_EARLY_MS - 1));
    assert!(in_window(x, x.at + x.up + HIT_LATE_MS));
    assert!(!in_window(x, x.at + x.up + HIT_LATE_MS + 1));
}

#[test]
fn score_never_pays_negative() {
    assert_eq!(settle_score(&[10, -25]), (-15, 0));
    assert_eq!(settle_score(&[10, 20, -25]), (5, 5));
    assert_eq!(settle_score(&[]), (0, 0));
}

#[test]
fn bomb_penalty_is_configurable() {
    let mut rng = StdRng::seed_from_u64(11);
    let s = build_schedule(&seeded(), TICKET, 40, &[1; 7], &mut rng);
    let bomb = s.iter().find(|x| x.idx < 0).expect("炸弹必有一枚");
    assert_eq!(bomb.value, -40);
}

#[test]
fn default_penalty_matches_public_const() {
    // 公示页读设置键的默认值，必须与排程默认同源
    assert_eq!(DEFAULT_BOMB_PENALTY, 25);
}
