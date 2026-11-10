//! economy_checkin 单测拆件（300 行门禁）：周期制里程碑/满周期归 1/
//! 配置周期跟随——见 economy_checkin.rs 的 cycle_days/cycle_pos。

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
fn milestones_cycle_after_full_period() {
    // 2026-11-10 循环制：满 30 天次日归位第 1 天，加送循环发。
    // 此前 31 天起永远 8 魔力——连签 400 天和 31 天拿得一样多。
    let p = CheckInParams::default();
    // 第二周期的里程碑日：绝对 40/50/60 = 周期内 10/20/30
    assert_eq!(checkin_reward(40, false, &p).total, 5 + 200);
    assert_eq!(checkin_reward(50, false, &p).total, 5 + 500);
    // 1005 触顶（单日封顶）
    assert_eq!(checkin_reward(60, false, &p).total, 1000);
    // 第二周期的非里程碑日（31..39 等）都只拿每日份
    for d in [31, 35, 39, 41] {
        assert_eq!(checkin_reward(d, false, &p).total, 5, "day {d}");
    }
    // 超长存量（线上最长 483）：周期位置 483%30=3，非里程碑日
    assert_eq!(p.cycle_pos(483), 3);
    assert_eq!(checkin_reward(483, false, &p).total, 5);
    // 487（位置 7）非里程碑、490（位置 10）命中 200
    assert_eq!(checkin_reward(487, false, &p).total, 5);
    assert_eq!(checkin_reward(490, false, &p).total, 5 + 200);
}
#[test]
fn cycle_days_follows_milestone_config() {
    // 周期 = 里程碑最大档，随配置自动变；空表回落 30
    let mut p = CheckInParams::default();
    assert_eq!(p.cycle_days(), 30);
    p.milestones = vec![(5, 100), (10, 300)];
    assert_eq!(p.cycle_days(), 10);
    assert_eq!(p.cycle_pos(11), 1); // 满 10 次日归 1
    p.milestones = vec![];
    assert_eq!(p.cycle_days(), 30);
    assert_eq!(p.cycle_pos(31), 1);
    // cycle_pos 恒为 1..=cycle
    for s in 1..=95 {
        let pos = p.cycle_pos(s);
        assert!((1..=30).contains(&pos), "streak {s} -> {pos}");
    }
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
        (
            "attendance_continuous_value".to_string(),
            "50,150".to_string(),
        ),
    ];
    let p = CheckInParams::from_settings(&rows);
    // 第 2 天命中配置里的里程碑（默认 streak=5 + 50）
    assert_eq!(checkin_reward(2, false, &p).total, 55);
    // 内置的 10/20/30 让位给配置表；周期=5，绝对第 10 天是第二周期的
    // 第 5 天——周期制下再次命中 150（旧断言 bonus=0 是一次性口径）
    assert_eq!(p.cycle_days(), 5);
    assert_eq!(checkin_reward(10, false, &p).streak_bonus, 150);
    // 周期第 1 天
    assert_eq!(checkin_reward(6, false, &p).streak_bonus, 0);
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
#[test]
fn shown_streak_decays_on_gap() {
    let today = chrono::NaiveDate::from_ymd_opt(2026, 9, 8).unwrap();
    let d = |n| Some((today - chrono::Duration::days(n), 12_i64));
    // 今天已签 / 昨天签过：链条还活着，照原计数展示
    assert_eq!(shown_streak(d(0), today), 12);
    assert_eq!(shown_streak(d(1), today), 12);
    // 断一天起必须归 0——旧写法把库里那个 12 原样端给首页和签到面板
    assert_eq!(shown_streak(d(2), today), 0);
    assert_eq!(shown_streak(d(30), today), 0);
    assert_eq!(shown_streak(None, today), 0);
    // 与签到主流程同一条边界（n=0 那档走不到：今日已签在落库前就被拒）。
    // 展示 >0 的那天签到落 13，展示 =0 的那天签到落 1。
    for n in 1..31 {
        let prev = d(n);
        let next = if (today - prev.unwrap().0).num_days() == 1 {
            prev.unwrap().1 + 1
        } else {
            1
        };
        let alive = shown_streak(prev, today) > 0;
        assert_eq!(next, if alive { 13 } else { 1 });
    }
}

#[test]
fn makeup_row_continues_streak() {
    // §9.3 修订：补签行参与续签。最后一条记录是**昨天的补签行**（历史重算已把
    // 其 streak 填成真实连续值，如 user 10556 的 11-08 补签 streak=214），今天
    // 签到应续到 215，而非被打回 1。
    let today = chrono::NaiveDate::from_ymd_opt(2026, 11, 9).unwrap();
    let yesterday = today - chrono::Duration::days(1);
    // shown_streak 只看 (日期, streak)，补签行与普通行一视同仁：
    // 昨天（补签）streak=214 → 展示仍活（214），今天签到 baseline 取 214 → 215。
    assert_eq!(shown_streak(Some((yesterday, 214)), today), 214);
    assert_eq!(shown_streak(Some((yesterday, 214)), today) + 1, 215);
}
