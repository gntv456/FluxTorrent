//! 打地鼠机制单测：锁的是**满盘闸**与排程形状，不是某张表。

use rand::rngs::StdRng;
use rand::SeedableRng;

use super::mole::{
    build_schedule, in_window, round_over, schedule_done, settle_score,
    settled_or_over, validate_mole, BOMB_SPAWNS, DEFAULT_BOMB_PENALTY,
    FIRST_SPAWN_MS, HIT_EARLY_MS, HIT_LATE_MS, POOL_SPAWNS, ROUND_MS,
    SPAWN_GAP_MS, UP_MS,
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

/// 排程没走完不许结算：切标签页 / 卸载会让前端提前调 finish，此前那一局
/// 当场按已命中的分数结算，后面没冒头的地鼠（含大奖）全部作废。
#[test]
fn schedule_is_not_done_until_last_window_closes() {
    let mut rng = StdRng::seed_from_u64(13);
    let s = build_schedule(&seeded(), TICKET, 25, &[1; 7], &mut rng);
    let last = s.last().expect("排程非空");
    let closes = last.at + last.up + HIT_LATE_MS;
    // 最后一只还在窗口里：没走完
    assert!(!schedule_done(&s, closes - 1));
    assert!(!schedule_done(&s, last.at));
    // 玩家玩到一半（第 3 只刚冒头）就切走：绝不能被结算
    assert!(!schedule_done(&s, s[2].at));
    // 窗口一关就算走完
    assert!(schedule_done(&s, closes));
    assert!(schedule_done(&s, closes + 1));
    // 比整局时长（ROUND_MS）早收尾：倒计时没走完也可以正常结算
    assert!(closes < ROUND_MS, "最后一只应在整局结束前收完：{closes}");
}

/// 排程解不出来（坏 JSON / 空排程被 `unwrap_or_default` 解成空 Vec）时：
/// `schedule_done` 自己永远不认，但整局时长闸必须在 ROUND_MS 后放行 ——
/// 否则这种局会卡成「永远结不了」的坏账，玩家已命中的分数被吞。
#[test]
fn broken_schedule_is_released_by_round_clock_only() {
    // 排程内容判据：空排程永不走完（不能把半局当 0 分结掉）
    assert!(!schedule_done(&[], i64::MAX));
    // 时长闸：整局一到就放行
    assert!(!round_over(ROUND_MS - 1));
    assert!(round_over(ROUND_MS));
    // 组合判据：半途一律拒，整局之后一律放行
    assert!(!settled_or_over(&[], 1_000));
    assert!(!settled_or_over(&[], ROUND_MS - 1));
    assert!(settled_or_over(&[], ROUND_MS));
    assert!(settled_or_over(&[], i64::MAX));
}

/// 正常排程走「内容优先」：最后一只一收尾（≈27.7s）就能结算，不必等满 30s。
#[test]
fn healthy_schedule_settles_before_round_clock() {
    let mut rng = StdRng::seed_from_u64(17);
    let s = build_schedule(&seeded(), TICKET, 25, &[1; 7], &mut rng);
    let closes = s.last().unwrap().at + UP_MS + HIT_LATE_MS;
    assert!(closes < ROUND_MS, "排程应早于整局结束：{closes}");
    // 内容判据单独放行
    assert!(schedule_done(&s, closes));
    // 时长闸此时尚未到点，组合判据靠内容判据放行
    assert!(!round_over(closes));
    assert!(settled_or_over(&s, closes));
    // 收尾前 1ms：两条判据都不认
    assert!(!settled_or_over(&s, closes - 1));
}

/// 固定节奏自检：最后一只的收尾时刻由四个常数推出，`round_over` 的
/// 「时长是不依赖排程内容的权威判据」这句依赖它成立。任何常数改动若让
/// 排程拖过整局时长，这里先红。
#[test]
fn schedule_always_ends_before_the_round_clock() {
    let last_at =
        FIRST_SPAWN_MS + (POOL_SPAWNS + BOMB_SPAWNS - 1) as i64 * SPAWN_GAP_MS;
    let closes = last_at + UP_MS + HIT_LATE_MS;
    assert!(
        closes < ROUND_MS,
        "最后一只收尾 {closes}ms 须早于整局 {ROUND_MS}ms，否则时长闸不再是上界"
    );
    let mut rng = StdRng::seed_from_u64(19);
    let s = build_schedule(&seeded(), TICKET, 25, &[1; 7], &mut rng);
    let real = s.last().unwrap().at + UP_MS + HIT_LATE_MS;
    assert_eq!(real, closes, "build_schedule 的节奏须与常数推导一致");
}
