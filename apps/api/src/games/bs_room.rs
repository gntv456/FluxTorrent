//! 猜大小「倒计时房间」机制：期次窗口、分区与派彩的**纯函数**层。
//!
//! 刻意不碰 DB、不拿时间源（`Utc::now`）：窗口剩余秒数由 HTTP 层把
//! `now - opened_at` 算成秒传进来，这里只做判定与算术，便于单测把边界锁死。
//! 经济口径（赢面/豹子判负/赔率 EV<1）沿用 `bigsmall.rs`，本模块只补房间侧。

use super::bigsmall::Guess;
use super::jgg::MULT_UNIT;

/// 缺省下注窗口（秒）。设置键 `bigsmall_round_seconds` 覆盖，钳在 10..=120。
pub const ROUND_SECONDS_DEFAULT: i64 = 30;
pub const ROUND_SECONDS_MIN: i64 = 10;
pub const ROUND_SECONDS_MAX: i64 = 120;

/// 读设置值后把窗口钳进合法区间；非法/缺失由调用方回落 `ROUND_SECONDS_DEFAULT`。
pub fn window_seconds(setting: i64) -> i64 {
    setting.clamp(ROUND_SECONDS_MIN, ROUND_SECONDS_MAX)
}

/// 距开奖剩余秒数。`elapsed` = now - opened_at（秒）；已过期返回 0，不出现负倒计时。
pub fn remaining_seconds(elapsed: i64, window: i64) -> i64 {
    (window - elapsed).max(0)
}

/// 三骰分区（不带玩家侧）：`triple` | `small` | `big`。
/// 小 = 点数和 3..=10，大 = 11..=18，三颗全同 = 豹子。
pub fn region_of(dice: [u32; 3]) -> &'static str {
    if dice[0] == dice[1] && dice[1] == dice[2] {
        return "triple";
    }
    if dice[0] + dice[1] + dice[2] <= 10 {
        "small"
    } else {
        "big"
    }
}

/// 玩家这一注是否命中：豹子通吃（押哪边都判负），其余按分区对侧。
pub fn side_wins(region: &str, side: &str) -> bool {
    match region {
        "small" => side == "small",
        "big" => side == "big",
        _ => false,
    }
}

/// 派彩：注额 × 千分赔率 / `MULT_UNIT`（1000），向下取整；乘法溢出饱和。
pub fn payout(amount: i64, win_mult_permille: i64) -> i64 {
    amount.saturating_mul(win_mult_permille) / MULT_UNIT
}

/// 解析下注侧（`small` | `big`）；非法返回 None。
pub fn parse_side(side: &str) -> Option<Guess> {
    match side {
        "small" => Some(Guess::Small),
        "big" => Some(Guess::Big),
        _ => None,
    }
}

/// 房间派彩的幂等键：`game-bs-win:<下注幂等键>`。
///
/// 前缀不是随便起的 —— `/games/rounds` 战绩口就是按
/// `'game-bs-win:' || l.idempotency_key` 把扣款流水 LEFT JOIN 到派彩流水上的。
/// 此前写成 `game-bs-round-win:<期>:<注>`，与扣款流水没有公共键，房间局在
/// 「记录」页永远匹配不到派彩（赢显示成输、连胜恒 0）。
pub fn win_idem(bet_idem: &str) -> String {
    format!("game-bs-win:{bet_idem}")
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn region_boundaries_three_dice() {
        assert_eq!(region_of([1, 1, 1]), "triple");
        assert_eq!(region_of([6, 6, 6]), "triple");
        // 和 10 = 小，和 11 = 大（机制边界）
        assert_eq!(region_of([1, 3, 6]), "small");
        assert_eq!(region_of([4, 3, 3]), "small");
        assert_eq!(region_of([4, 4, 3]), "big");
        assert_eq!(region_of([3, 4, 5]), "big");
    }

    #[test]
    fn triple_loses_on_both_sides() {
        assert!(!side_wins("triple", "small"));
        assert!(!side_wins("triple", "big"));
        assert!(side_wins("small", "small"));
        assert!(!side_wins("small", "big"));
        assert!(side_wins("big", "big"));
        assert!(!side_wins("big", "small"));
    }

    #[test]
    fn payout_floors_and_saturates() {
        assert_eq!(payout(1000, 1900), 1900);
        assert_eq!(payout(100, 1900), 190);
        // 101 × 1.9 = 191.9 → 向下取整 191
        assert_eq!(payout(101, 1900), 191);
        // 豹子/未中走 0 倍率
        assert_eq!(payout(1000, 0), 0);
    }

    #[test]
    fn window_is_clamped() {
        assert_eq!(window_seconds(5), ROUND_SECONDS_MIN);
        assert_eq!(window_seconds(30), 30);
        assert_eq!(window_seconds(200), ROUND_SECONDS_MAX);
    }

    #[test]
    fn remaining_never_negative() {
        assert_eq!(remaining_seconds(20, 30), 10);
        assert_eq!(remaining_seconds(0, 30), 30);
        assert_eq!(remaining_seconds(40, 30), 0);
    }

    #[test]
    fn room_ev_stays_below_one() {
        // 房间制与即时局同一赔率口径，回收口纪律不变
        let ev = super::super::bigsmall_expected_value(1900);
        assert!(ev < 1.0, "房间派彩 EV 必须 < 1，实得 {ev}");
    }

    /// 派彩键必须能被战绩口 JOIN 到：`/games/rounds` 的条件是
    /// `'game-bs-win:' || <扣款流水 idem>`。两边任何一边改了前缀，房间局
    /// 就会从「记录」里消失（此前就是这样）。
    #[test]
    fn win_idem_matches_the_rounds_history_join() {
        let bet = "game-bs-room:42:abc";
        let win = win_idem(bet);
        assert_eq!(win, format!("game-bs-win:{bet}"));
        assert!(win.starts_with("game-bs-win:"), "战绩口只认这个前缀：{win}");
        assert!(!win.contains("round-win"), "旧前缀 JOIN 不到，不许回退");
    }

    /// `insert_bet` 的占位符个数必须等于 `.bind()` 个数。
    ///
    /// 这是一条**源码级守卫**：动态 SQL 的参数数量不符，编译器和类型系统都
    /// 抓不到，只有线上执行时才炸。此前 INSERT 声明了 `$1..$5` 却只 bind 了
    /// 4 个 —— PG 报 `Expected 5 parameters but got 4`，整笔回滚，房间下注
    /// **自上线起一注都没成功过**（本地库 22 期 / 0 注 / 0 流水可复现）。
    #[test]
    fn insert_bet_binds_every_placeholder() {
        const SRC: &str = include_str!("../games_http/bs_room_store.rs");
        let start = SRC
            .find("INSERT INTO arcade_bigsmall_bets")
            .expect("insert_bet 的 INSERT 语句应存在");
        // 语句 = 从 INSERT 到 `.execute(`；绑定链夹在两者之间
        let exec = start + SRC[start..].find(".execute(").unwrap();
        let stmt = &SRC[start..exec];
        let placeholders = stmt.matches('$').count();
        let binds = stmt.matches(".bind(").count();
        assert_eq!(
            placeholders, binds,
            "占位符 {placeholders} 个但只 bind 了 {binds} 个：这条 INSERT 线上必败"
        );
        assert_eq!(
            placeholders, 5,
            "列：round_id/user_id/side/amount/idempotency_key"
        );
    }
}
