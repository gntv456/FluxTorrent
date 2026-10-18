//! 勋章成就判定的单测（配错规则会「永远达不成」或「人人秒达成」）。

use super::*;

fn ev(target: i64) -> Rule {
    Rule {
        mode: Mode::Event,
        period: Period::Total,
        target,
    }
}
fn th(target: i64) -> Rule {
    Rule {
        mode: Mode::Threshold,
        period: Period::Total,
        target,
    }
}

/// 模式解析：未知值一律回落 Manual（宁可不发也不发错）。
#[test]
fn mode_parse_is_conservative() {
    assert_eq!(Mode::parse(Some("event")), Mode::Event);
    assert_eq!(Mode::parse(Some("threshold")), Mode::Threshold);
    assert_eq!(Mode::parse(None), Mode::Manual);
    assert_eq!(Mode::parse(Some("")), Mode::Manual);
    assert_eq!(Mode::parse(Some("乱配")), Mode::Manual);
}

/// 周期解析：未知值回落 Total（周期配错不该让阈值口径变松）。
#[test]
fn period_parse_falls_back_to_total() {
    assert_eq!(Period::parse(Some("day")), Period::Day);
    assert_eq!(Period::parse(Some("month")), Period::Month);
    assert_eq!(Period::parse(Some("unknown")), Period::Total);
    assert_eq!(Period::parse(None), Period::Total);
}

/// 事件累加：未达标推进进度。
#[test]
fn event_accumulates_progress() {
    assert_eq!(
        judge_event(&ev(100), 30, 10),
        Verdict::Progressed { current: 40 }
    );
}

/// 事件累加：达标即 Award。
#[test]
fn event_awards_at_target() {
    assert_eq!(judge_event(&ev(100), 95, 5), Verdict::Award);
}

/// 负 amount 必须被夹成 0（负数事件不应让进度倒退）。
#[test]
fn negative_amount_clamped() {
    assert_eq!(
        judge_event(&ev(100), 50, -20),
        Verdict::Progressed { current: 50 }
    );
}

/// 进度允许超过 target（进度条要显示真实值），判定只看 >=。
#[test]
fn current_may_exceed_target() {
    assert_eq!(judge_event(&ev(100), 150, 10), Verdict::Award);
}

/// 非 event 模式走 event 判定 → Pending。
#[test]
fn event_judge_ignores_non_event_mode() {
    assert_eq!(judge_event(&th(100), 0, 10), Verdict::Pending);
}

/// threshold：整体重算，无增量。
#[test]
fn threshold_is_absolute_not_incremental() {
    assert_eq!(
        judge_threshold(&th(50), 30, false),
        Verdict::Progressed { current: 30 }
    );
    assert_eq!(judge_threshold(&th(50), 50, false), Verdict::Award);
}

/// 已授予不再重复发（threshold 模式）。
#[test]
fn threshold_already_awarded_is_noop() {
    assert_eq!(judge_threshold(&th(50), 99, true), Verdict::AlreadyAwarded);
}

/// 进度比例封顶 1.0——超额不该让进度条画出界。
#[test]
fn progress_ratio_clamps() {
    assert_eq!(progress_ratio(50, 100), 0.5);
    assert_eq!(progress_ratio(250, 100), 1.0);
    assert_eq!(progress_ratio(0, 100), 0.0);
    // target 配 0：视为零门槛，给满
    assert_eq!(progress_ratio(5, 0), 1.0);
}

/// 进度文案统一口径（避免各处自己拼出矛盾读数）。
#[test]
fn progress_text_unified() {
    assert_eq!(progress_text(50, 100, "小时"), "50 / 100 小时");
    assert_eq!(progress_text(50, 100, ""), "50 / 100");
}

/// 配置校验：target<=0 与 threshold 缺周期都要拦。
#[test]
fn validate_rule_rejects_bad_config() {
    assert!(validate_rule(Mode::Manual, 0, None).is_ok());
    assert!(validate_rule(Mode::Event, 100, None).is_ok());
    assert!(validate_rule(Mode::Event, 0, None).is_err());
    assert!(validate_rule(Mode::Event, -1, None).is_err());
    assert!(validate_rule(Mode::Threshold, 100, None).is_err());
    assert!(validate_rule(Mode::Threshold, 100, Some(Period::Total)).is_ok());
}

/// 周期窗口天数：Week/Month 取「本周期已过天数」。
#[test]
fn period_window_days() {
    use chrono::TimeZone;
    // 2026-10-16 是周五（weekday=4，周一起算）
    let fri = chrono::Utc.with_ymd_and_hms(2026, 10, 16, 0, 0, 0).unwrap();
    assert_eq!(Period::Total.window_days(fri), 0);
    assert_eq!(Period::Day.window_days(fri), 0);
    assert_eq!(Period::Week.window_days(fri), 4);
    assert_eq!(Period::Month.window_days(fri), 15);
}
