//! 签到规则（M12 旧站口径，从 economy.rs 按域拆出，300 门禁）。
//!
//! 奖励参数全部走 site_settings：`attendance_first` / `attendance_streak` /
//! `attendance_daily_cap` 三个直读键，加 `attendance_continuous_day` /
//! `_value` 两个里程碑列表键（P-97 之前里程碑硬编码在本文件的 match 里，
//! 后台同名配置键零消费者）。脏值一律回落内置默认，不让签到整条链路失败。

// ============ 签到规则（M12 旧站口径） ============

/// 一次签到的奖励拆解。
#[derive(Debug, PartialEq)]
pub struct CheckInReward {
    pub base: i64,
    pub streak_bonus: i64,
    pub total: i64,
    pub streak: i64,
}

/// 签到奖励参数（0109 的 `attendance_*` 设置键）。
#[derive(Debug, Clone, PartialEq)]
pub struct CheckInParams {
    pub first: i64,
    pub streak: i64,
    pub daily_cap: i64,
    /// 连签里程碑（第几天, 加送多少）。P-97 之前里程碑硬编码在
    /// `checkin_reward` 的 match 里，而后台同时摆着
    /// `attendance_continuous_day` / `attendance_continuous_value`
    /// 两个「看起来就是它」的键——站长改了不生效。现按这两键读取，
    /// 缺省仍为 10/20/30 → 200/500/1000。
    pub milestones: Vec<(i64, i64)>,
}

impl Default for CheckInParams {
    fn default() -> Self {
        Self {
            first: 10,
            streak: 5,
            daily_cap: 1000,
            milestones: vec![(10, 200), (20, 500), (30, 1000)],
        }
    }
}

/// 逗号分隔列表（`"10,20,30"`）→ 正整数；脏值/非正数直接丢掉，
/// 由调用方按「两边条数不齐即回落默认」判定。
fn parse_id_list(raw: &str) -> Vec<i64> {
    raw.split(',')
        .filter_map(|x| x.trim().parse::<i64>().ok())
        .filter(|n| *n > 0)
        .collect()
}

impl CheckInParams {
    /// 用 site_settings 的现有值覆盖默认。缺键、空值、非数字一律回落默认——
    /// 脏配置不该让签到整条链路失败。
    pub fn from_settings(rows: &[(String, String)]) -> Self {
        let mut p = Self::default();
        let mut days: Option<String> = None;
        let mut vals: Option<String> = None;
        for (k, v) in rows {
            let Ok(n) = v.trim().parse::<i64>() else {
                match k.as_str() {
                    "attendance_continuous_day" => days = Some(v.clone()),
                    "attendance_continuous_value" => vals = Some(v.clone()),
                    _ => {}
                }
                continue;
            };
            match k.as_str() {
                "attendance_first" => p.first = n,
                "attendance_streak" => p.streak = n,
                "attendance_daily_cap" => p.daily_cap = n,
                _ => {}
            }
        }
        if let (Some(d), Some(v)) = (days, vals) {
            let (ds, vs) = (parse_id_list(&d), parse_id_list(&v));
            if !ds.is_empty() && ds.len() == vs.len() {
                p.milestones = ds.into_iter().zip(vs).collect();
            }
        }
        p
    }
}

/// 签到奖励：首次 +first；连签每日 +streak（整日封顶 daily_cap）；
/// 命中连签里程碑（`attendance_continuous_day`，缺省 10/20/30）当天
/// 额外加送对应 value（缺省 200/500/1000）。
pub fn checkin_reward(
    streak_after: i64,
    is_first_ever: bool,
    params: &CheckInParams,
) -> CheckInReward {
    let base = if is_first_ever {
        params.first
    } else {
        params.streak
    };
    // 连签额外奖励按里程碑判断（在达到该里程碑的当天发放）
    let streak_bonus = params
        .milestones
        .iter()
        .find(|(day, _)| *day == streak_after)
        .map(|(_, bonus)| *bonus)
        .unwrap_or(0);
    let total = (base + streak_bonus).min(params.daily_cap);
    CheckInReward {
        base,
        streak_bonus,
        total,
        streak: streak_after,
    }
}

/// 连签计算：昨天有记录 → 连签+1；否则重置为 1
#[allow(dead_code)]
pub fn next_streak(
    last_date: Option<chrono::NaiveDate>,
    today: chrono::NaiveDate,
) -> i64 {
    match last_date {
        Some(d) if (today - d).num_days() == 1 => 2, // 调用方在此基础上 + 原连签数
        _ => 1,
    }
}


#[cfg(test)]
mod tests {
    use super::*;


    #[test]
    fn checkin_first_ever() {
        let r = checkin_reward(1, true, &CheckInParams::default());
        assert_eq!(r.total, 10);
    }
    #[test]
    fn checkin_streak_milestones() {
        let p = CheckInParams::default();
        assert_eq!(checkin_reward(10, false, &p).total, 5 + 200);
        assert_eq!(checkin_reward(20, false, &p).total, 5 + 500);
        // 连签 30 天：5 + 1000 = 1005，但单日封顶 1000（旧站口径）
        assert_eq!(checkin_reward(30, false, &p).total, 1000);
        assert_eq!(checkin_reward(11, false, &p).total, 5); // 非里程碑日
    }
    #[test]
    fn checkin_cap_1000() {
        // 连签 30 天且首签叠加场景也不会超过封顶
        let r = checkin_reward(30, true, &CheckInParams::default());
        assert!(r.total <= 1000);
    }
    #[test]
    fn checkin_params_come_from_settings() {
        let rows = vec![
            ("attendance_first".to_string(), "30".to_string()),
            ("attendance_streak".to_string(), "8".to_string()),
            ("attendance_daily_cap".to_string(), "500".to_string()),
        ];
        let p = CheckInParams::from_settings(&rows);
        assert_eq!(checkin_reward(1, true, &p).total, 30);
        assert_eq!(checkin_reward(11, false, &p).total, 8);
        // 里程碑 200 + 每日 8 = 208 < 500 未触顶；30 天里程碑 1000 被日封顶截断
        assert_eq!(checkin_reward(30, false, &p).total, 500);
        // 脏值/缺键回落默认
        let dirty = vec![("attendance_first".to_string(), "".to_string())];
        assert_eq!(CheckInParams::from_settings(&dirty).first, 10);
    }
    #[test]
    fn checkin_milestones_come_from_settings() {
        let rows = vec![
            ("attendance_continuous_day".to_string(), "2,5".to_string()),
            ("attendance_continuous_value".to_string(), "50,150".to_string()),
        ];
        let p = CheckInParams::from_settings(&rows);
        // 第 2 天命中配置里的里程碑（默认 streak=5 + 50）
        assert_eq!(checkin_reward(2, false, &p).total, 55);
        // 内置的 10/20/30 让位给配置表
        assert_eq!(checkin_reward(10, false, &p).streak_bonus, 0);
        // 两边条数不齐 / 全脏 → 回落内置里程碑
        let bad = vec![
            ("attendance_continuous_day".to_string(), "10,20".to_string()),
            ("attendance_continuous_value".to_string(), "200".to_string()),
        ];
        let q = CheckInParams::from_settings(&bad);
        assert_eq!(checkin_reward(20, false, &q).streak_bonus, 500);
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
}
