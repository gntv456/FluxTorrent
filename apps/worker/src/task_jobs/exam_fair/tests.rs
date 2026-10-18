//! 考核阶段状态机的穷举单测（误封用户 / 放水作弊是这里最坏的后果）。

use super::*;

fn inp(
    seq: i32,
    is_last: bool,
    dur: i32,
    rem: i32,
    act: FailAction,
    elapsed: i32,
    met: bool,
) -> StageInput {
    StageInput {
        seq,
        is_last,
        duration_days: dur,
        remedy_days: rem,
        fail_action: act,
        elapsed_days: elapsed,
        met,
    }
}

/// 未到期 → 进行中，并给出剩余天数。
#[test]
fn before_deadline_in_progress() {
    let v = judge_stage(&inp(1, false, 7, 2, FailAction::None, 3, false));
    assert_eq!(v, StageVerdict::InProgress { remain_days: 4 });
}

/// 达标优先于时间：卡点当天达标也算过，不给「差一分钟」添堵。
#[test]
fn met_beats_deadline() {
    // 恰好到期（elapsed == duration）
    let v = judge_stage(&inp(1, true, 7, 0, FailAction::Ban, 7, true));
    assert_eq!(v, StageVerdict::StagePassed { exam_passed: true });
    // 严重逾期后才达标也要算过（补救期内补齐）
    let v = judge_stage(&inp(1, true, 7, 3, FailAction::Ban, 9, true));
    assert_eq!(v, StageVerdict::StagePassed { exam_passed: true });
}

/// 到期未达标 + 有补救期 → 进补救期，不判失败。
#[test]
fn overdue_enters_remedy_not_fail() {
    let v = judge_stage(&inp(1, false, 7, 3, FailAction::Ban, 8, false));
    assert_eq!(
        v,
        StageVerdict::InRemedy {
            remedy_remain_days: 2,
            overdue_days: 1,
        }
    );
}

/// 补救期也过了 → 才失败并带处置动作。
#[test]
fn after_remedy_fails() {
    let v = judge_stage(&inp(1, false, 7, 3, FailAction::Ban, 10, false));
    assert_eq!(
        v,
        StageVerdict::StageFailed {
            action: FailAction::Ban
        }
    );
}

/// 无补救期（remedy=0）→ 到期直接失败（向后兼容旧行为）。
#[test]
fn no_remedy_fails_directly() {
    let v = judge_stage(&inp(1, false, 7, 0, FailAction::None, 7, false));
    assert_eq!(
        v,
        StageVerdict::StageFailed {
            action: FailAction::None
        }
    );
}

/// 非最后一段达标 → 进下一段（不是整体通过）。
#[test]
fn mid_stage_advances() {
    let v = judge_stage(&inp(2, false, 7, 0, FailAction::None, 3, true));
    assert_eq!(
        progress(&v, 2),
        Progress::Advance {
            next_seq: 3,
            next_is_last: false
        }
    );
}

/// 最后一段达标 → 整体通过。
#[test]
fn last_stage_passes_exam() {
    let v = judge_stage(&inp(3, true, 7, 0, FailAction::None, 1, true));
    assert_eq!(progress(&v, 3), Progress::ExamPassed);
}

/// 补救期本身不产出推进动作（worker 无需写库，只等下一轮）。
#[test]
fn remedy_yields_no_progress_action() {
    let v = judge_stage(&inp(1, false, 7, 3, FailAction::None, 8, false));
    assert_eq!(progress(&v, 1), Progress::None);
}

/// FailAction 解析：未知值回落 None（不处置），永不意外封人。
#[test]
fn fail_action_parse_is_conservative() {
    assert_eq!(FailAction::parse("ban"), FailAction::Ban);
    assert_eq!(FailAction::parse("restrict"), FailAction::Restrict);
    assert_eq!(FailAction::parse("none"), FailAction::None);
    assert_eq!(FailAction::parse("未知"), FailAction::None);
    assert_eq!(FailAction::parse(""), FailAction::None);
}

/// 补救期建议长度：取 80 分位；空输入无建议。
#[test]
fn suggest_remedy_uses_p80() {
    assert_eq!(suggest_remedy_days(&[]), None);
    // 1..10 的 80 分位 → 第 8 个（idx = ceil(10*0.8)-1 = 7 → 值 8）
    let days: Vec<i32> = (1..=10).collect();
    assert_eq!(suggest_remedy_days(&days), Some(8));
    // 乱序也要对
    let mut shuffled = days.clone();
    shuffled.reverse();
    assert_eq!(suggest_remedy_days(&shuffled), Some(8));
    // 单点
    assert_eq!(suggest_remedy_days(&[3]), Some(3));
    // 全 0 → 0（不需要补救期）
    assert_eq!(suggest_remedy_days(&[0, 0, 0]), Some(0));
}

/// 阶段配置校验：配错会让人卡死，必须拦。
#[test]
fn validate_stage_rejects_bad_config() {
    assert!(validate_stage(7, 3, 1).is_ok());
    assert!(validate_stage(0, 0, 0).is_err(), "时限必须为正");
    assert!(validate_stage(-1, 0, 0).is_err());
    assert!(validate_stage(7, -1, 0).is_err(), "补救期不能为负");
    assert!(validate_stage(7, 100, 0).is_err(), "补救期超过时限 3 倍");
    assert!(validate_stage(7, 0, -1).is_err(), "延期次数不能为负");
}

/// 边界：补救期恰好等于逾期天数时仍在宽限内（inclusive）。
#[test]
fn remedy_boundary_is_inclusive() {
    // overdue == remedy_days → 刚好用尽宽限，下一轮才失败
    let v = judge_stage(&inp(1, false, 7, 3, FailAction::Ban, 10, false));
    assert_eq!(
        v,
        StageVerdict::StageFailed {
            action: FailAction::Ban
        }
    );
    // overdue == remedy_days - 1 → 还在宽限
    let v = judge_stage(&inp(1, false, 7, 3, FailAction::Ban, 9, false));
    assert!(matches!(v, StageVerdict::InRemedy { .. }));
}
