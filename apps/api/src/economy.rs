//! 火花经济领域层（M11/M12，方案 §4.2 旧站口径）。
//! 纪律（§5.6）：一切动账经 ledger 流水；余额只是快照。

use chrono::{Datelike, Duration, Utc};

// ============ 签到规则（M12 旧站口径） ============

/// 签到奖励：首次 +10；连签每日 +5（封顶 1000）；连签 10/20/30 天额外 200/500/1000。
#[derive(Debug, PartialEq)]
pub struct CheckInReward {
    pub base: i64,
    pub streak_bonus: i64,
    pub total: i64,
    pub streak: i64,
}

pub fn checkin_reward(streak_after: i64, is_first_ever: bool) -> CheckInReward {
    let base = if is_first_ever { 10 } else { 5 };
    // 连签额外奖励按里程碑判断（在 streak_after 达到 10/20/30 当天发放）
    let streak_bonus = match streak_after {
        30 => 1000,
        20 => 500,
        10 => 200,
        _ => 0,
    };
    let total = (base + streak_bonus).min(1000);
    CheckInReward {
        base,
        streak_bonus,
        total,
        streak: streak_after,
    }
}

/// 连签计算：昨天有记录 → 连签+1；否则重置为 1
#[allow(dead_code)]
pub fn next_streak(last_date: Option<chrono::NaiveDate>, today: chrono::NaiveDate) -> i64 {
    match last_date {
        Some(d) if (today - d).num_days() == 1 => 2, // 调用方在此基础上 + 原连签数
        _ => 1,
    }
}

// ============ 银行规则（M11 旧站口径：7/30/90/180/365 天定期） ============

/// 定期利率（年化，旧站口径）
pub fn term_rate(term_days: i32) -> f64 {
    match term_days {
        7 => 0.01,
        30 => 0.03,
        90 => 0.06,
        180 => 0.10,
        365 => 0.18,
        _ => 0.0,
    }
}

/// 到期利息（整数火花，向下取整防超发）
pub fn maturity_interest(principal: i64, term_days: i32) -> i64 {
    let rate = term_rate(term_days);
    ((principal as f64) * rate * (term_days as f64) / 365.0).floor() as i64
}

#[allow(dead_code)]
pub fn maturity_date(start: chrono::DateTime<Utc>, term_days: i32) -> chrono::DateTime<Utc> {
    start + Duration::days(term_days as i64)
}

pub const VALID_TERMS: [i32; 5] = [7, 30, 90, 180, 365];

// ============ 站免池（M13 旧站口径：月累计 200 万触发次月全局双免） ============

pub const MAGIC_POOL_GOAL: i64 = 2_000_000;

pub fn pool_month(t: chrono::DateTime<Utc>) -> String {
    format!("{:04}-{:02}", t.year(), t.month())
}

/// 上月达标 → 本月 1-3 号开启全局双免
#[allow(dead_code)]
pub fn pool_promo_window(
    now: chrono::DateTime<Utc>,
) -> Option<(chrono::DateTime<Utc>, chrono::DateTime<Utc>)> {
    let day = now.day();
    if (1..=3).contains(&day) {
        let month_start = now.date_naive().and_hms_opt(0, 0, 0)?;
        let start = chrono::DateTime::from_naive_utc_and_offset(month_start, Utc);
        Some((start, start + Duration::days(3)))
    } else {
        None
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn checkin_first_ever() {
        let r = checkin_reward(1, true);
        assert_eq!(r.total, 10);
    }

    #[test]
    fn checkin_streak_milestones() {
        assert_eq!(checkin_reward(10, false).total, 5 + 200);
        assert_eq!(checkin_reward(20, false).total, 5 + 500);
        // 连签 30 天：5 + 1000 = 1005，但单日封顶 1000（旧站口径）
        assert_eq!(checkin_reward(30, false).total, 1000);
        assert_eq!(checkin_reward(11, false).total, 5); // 非里程碑日
    }

    #[test]
    fn checkin_cap_1000() {
        // 连签 30 天且首签叠加场景也不会超过封顶
        let r = checkin_reward(30, true);
        assert!(r.total <= 1000);
    }

    #[test]
    fn streak_break_and_continue() {
        let today = chrono::NaiveDate::from_ymd_opt(2026, 9, 8).unwrap();
        let yesterday = today - chrono::Duration::days(1);
        assert_eq!(next_streak(Some(yesterday), today), 2);
        assert_eq!(
            next_streak(Some(today - chrono::Duration::days(2)), today),
            1
        );
        assert_eq!(next_streak(None, today), 1);
    }

    #[test]
    fn interest_math() {
        // 10000 火花存 365 天 @18%：10000*0.18 = 1800
        assert_eq!(maturity_interest(10000, 365), 1800);
        // 10000 存 30 天 @3%：10000*0.03*30/365 = 24.65 → 24
        assert_eq!(maturity_interest(10000, 30), 24);
        assert_eq!(maturity_interest(0, 365), 0);
    }

    #[test]
    fn invalid_term_zero_rate() {
        assert_eq!(term_rate(45), 0.0);
        assert_eq!(maturity_interest(10000, 45), 0);
    }

    #[test]
    fn pool_month_format() {
        let t = chrono::NaiveDate::from_ymd_opt(2026, 9, 15)
            .unwrap()
            .and_hms_opt(0, 0, 0)
            .unwrap();
        assert_eq!(
            pool_month(chrono::DateTime::from_naive_utc_and_offset(t, Utc)),
            "2026-09"
        );
    }

    #[test]
    fn pool_window_only_first_3_days() {
        let d1 = chrono::NaiveDate::from_ymd_opt(2026, 10, 1)
            .unwrap()
            .and_hms_opt(12, 0, 0)
            .unwrap();
        assert!(pool_promo_window(chrono::DateTime::from_naive_utc_and_offset(d1, Utc)).is_some());
        let d5 = chrono::NaiveDate::from_ymd_opt(2026, 10, 5)
            .unwrap()
            .and_hms_opt(12, 0, 0)
            .unwrap();
        assert!(pool_promo_window(chrono::DateTime::from_naive_utc_and_offset(d5, Utc)).is_none());
    }
}
