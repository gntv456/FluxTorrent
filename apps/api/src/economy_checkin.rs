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

impl CheckInParams {
    /// 连签周期长度（站长口径 2026-11-10）：取里程碑最大档。满一个周期后
    /// 次日从第 1 天重计，10/20/30 的加送循环可反复拿——30 天后永远 8 魔力
    /// 的话，连签 400 天和 31 天拿得一样多，保连签只剩「怕断」没有奔头。
    /// 站长改 `attendance_continuous_day` 加/减档，周期自动跟随，无独立键。
    /// 空里程碑表回落 30（与缺省里程碑最大档一致）。
    pub fn cycle_days(&self) -> i64 {
        self.milestones.iter().map(|(d, _)| *d).max().unwrap_or(30)
    }

    /// 绝对连签数 → 周期内位置（1..=cycle）。存量超长连签（线上 1972 人
    /// streak>30，最长 483）不回刷：483 天的人 `(483 % 30)=3`，当前周期
    /// 走到第 3 天，下个加送档在第 10 天（绝对 490）。
    pub fn cycle_pos(&self, streak: i64) -> i64 {
        let c = self.cycle_days();
        if c <= 0 {
            return streak.max(1);
        }
        let pos = (streak - 1) % c + 1;
        pos.max(1)
    }
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
///
/// 周期制（2026-11-10）：里程碑按**周期内位置**判定而非绝对天数——满周期
/// （最大档 30 天）后次日归位第 1 天，10/20/30 的加送循环可反复拿。
/// `streak` 字段落库仍是绝对累计值（不破坏历史数据与展示口径）。
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
    // 连签额外奖励：按周期内位置命中里程碑（达到该档的当天发放，满周期循环）
    let streak_bonus = params
        .milestones
        .iter()
        .find(|(day, _)| *day == params.cycle_pos(streak_after))
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

/// 展示用连签数（P-471）：最后一条记录落在**今天或昨天**才叫链条还活着，
/// 照原计数展示；断了一天及以上 → 0。
///
/// 断链边界必须与签到主流程一致（checkin.rs 里 `prev_date == Some(yesterday)`
/// 那条），否则会员在首页看到「连签 12 天」、点下去回的是 `streak=1`——展示口
/// 各写一份时忘了衰减，就是这个形状。
///
/// §9.3 修订：**补签行（makeup）也参与续签**——补签是用户花卡补的漏签日，理应
/// 让连签接得上；历史重算已把补签行的 streak 填成真实连续值（不是 0）。因此
/// 调用方取「最后一条记录」即可，不再排除 makeup。
pub fn shown_streak(
    last: Option<(chrono::NaiveDate, i64)>,
    today: chrono::NaiveDate,
) -> i64 {
    match last {
        Some((d, s)) if (today - d).num_days() <= 1 => s,
        _ => 0,
    }
}
#[cfg(test)]
#[path = "economy_checkin_tests.rs"]
mod tests;
