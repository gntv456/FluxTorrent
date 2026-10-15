//! 考核多阶段推进的判定（PT Studio FR-EXAM-001~017 对标，FRAME §7 第 6 条）。
//!
//! 现状（0093 单段）：到期即判「完成 / 失败」二选一。缺三样：
//! 多阶段（逐段推进、每段自己的时限与目标）、补救期（逾期先宽限，
//! 期内补齐即算通过）、阶段级基线（第 N 段从第 N-1 段末尾起算）。
//!
//! 【为什么状态机判定必须在 domain】结算循环每分钟跑一次，一次判错
//! 就是「误封用户」或「放水让作弊者过关」。状态迁移矩阵是纯逻辑，
//! 放进单测里穷举比放在 worker 里靠现网反馈靠谱。
//!
//! 【最重要的设计：补救期不是第二次宽刑】
//! 阶段到期未达标时**不立刻判失败**，而是进入补救期：
//!   * 补救期内补齐 → 复活，视为达标，且记 `exam_remedy_log`（用于校准）；
//!   * 补救期也过了 → 才按 `fail_action` 处置。
//! 这样「出差一周回来就被封」的申诉成本被消掉，同时没有放松真实门槛。

use crate::errors::{DomainError, DomainResult};

/// 未达标时的处置动作。
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum FailAction {
    /// 不处置，只标记失败。
    None,
    /// 限制下载。
    Restrict,
    /// 封禁。
    Ban,
}

impl FailAction {
    pub fn parse(s: &str) -> Self {
        match s {
            "restrict" => Self::Restrict,
            "ban" => Self::Ban,
            _ => Self::None,
        }
    }
}

/// 一阶段的状态机输入。
#[derive(Debug, Clone, Copy)]
pub struct StageInput {
    /// 阶段 seq（从 1 起）。
    pub seq: i32,
    /// 本阶段是否已是最后一段。
    pub is_last: bool,
    /// 本阶段时限（天）。
    pub duration_days: i32,
    /// 补救期天数（0 = 无补救，到期直接判失败）。
    pub remedy_days: i32,
    /// 逾期处置。
    pub fail_action: FailAction,
    /// 距本阶段开始已过天数（含小数，向上取整判读）。
    pub elapsed_days: i32,
    /// 全部目标是否已达标。
    pub met: bool,
}

/// 本阶段的判定结果。
#[derive(Debug, Clone, PartialEq)]
pub enum StageVerdict {
    /// 继续本阶段（尚未到期）。
    InProgress {
        /// 距本阶段截止还有几天（>=0）。
        remain_days: i32,
    },
    /// 本阶段达标：要么进下一阶段，要么整体通过。
    StagePassed {
        /// 是否整体通过（本次达标即最后一段）。
        exam_passed: bool,
    },
    /// 本阶段未达标但仍在补救期内——**不判失败**，给一条不用申诉的路。
    InRemedy {
        /// 距补救期截止还有几天。
        remedy_remain_days: i32,
        /// 逾期天数（已超原时限的天数），写日志用。
        overdue_days: i32,
    },
    /// 本阶段最终失败（含补救期也过了），按 fail_action 处置。
    StageFailed {
        action: FailAction,
    },
}

/// 判定单阶段状态。
///
/// 时序口径（`elapsed_days` 以天为单位、向上取整判读）：
///   * `elapsed_days < duration_days` → 进行中；
///   * 达标优先于时间：**已达标哪怕刚好到期也算过**（不给「差一分钟」
///     的用户添堵，这是同 exams 的到期判定口径）；
///   * 未达标且已到期 → 有补救期则进 InRemedy，否则 StageFailed。
pub fn judge_stage(i: &StageInput) -> StageVerdict {
    // 达标优先：只要达标就推进，不管是不是刚好卡点。
    if i.met {
        return StageVerdict::StagePassed {
            exam_passed: i.is_last,
        };
    }
    if i.elapsed_days < i.duration_days {
        return StageVerdict::InProgress {
            remain_days: i.duration_days - i.elapsed_days,
        };
    }
    // 到期未达标：先进补救期（若有）
    let overdue = i.elapsed_days - i.duration_days;
    if i.remedy_days > 0 && overdue < i.remedy_days {
        return StageVerdict::InRemedy {
            remedy_remain_days: i.remedy_days - overdue,
            overdue_days: overdue,
        };
    }
    StageVerdict::StageFailed {
        action: i.fail_action,
    }
}

/// 一次结算的整体推进结果（供 worker 落库）。
#[derive(Debug, Clone, PartialEq)]
pub enum Progress {
    /// 未到判定点，什么都不做（省一次 UPDATE）。
    None,
    /// 阶段通过，进入下一阶段（不是最后一段）。
    Advance {
        next_seq: i32,
        next_is_last: bool,
    },
    /// 整体通过（最后一段达标）。
    ExamPassed,
    /// 进入补救期。
    EnterRemedy {
        remedy_until_days: i32,
        overdue_days: i32,
    },
    /// 阶段失败。
    Failed {
        action: FailAction,
    },
}

/// 由单阶段判定推出整体推进。
pub fn progress(v: &StageVerdict, seq: i32, is_last: bool) -> Progress {
    match v {
        StageVerdict::InProgress { .. } | StageVerdict::InRemedy { .. } => {
            Progress::None
        }
        StageVerdict::StagePassed { exam_passed } => {
            if *exam_passed {
                Progress::ExamPassed
            } else {
                Progress::Advance {
                    next_seq: seq + 1,
                    next_is_last: false,
                }
            }
        }
        StageVerdict::StageFailed { action } => {
            Progress::Failed { action: *action }
        }
    }
}

/// 校准补救期长度：给定一批逾期天数，给出「能救回几成」的建议天数。
///
/// 这是 `exam_remedy_log` 的用法：若绝大多数人逾期 1-2 天内补齐，
/// 设 0 天就是纯粹制造申诉而非真实门槛。
pub fn suggest_remedy_days(overdue_days: &[i32]) -> Option<i32> {
    if overdue_days.is_empty() {
        return None;
    }
    let mut v = overdue_days.to_vec();
    v.sort_unstable();
    // 取 80 分位：覆盖大多数人的逾期，不为极少数极端值开太宽的口子
    let idx = ((v.len() as f64) * 0.8).ceil() as usize - 1;
    Some(v[idx.min(v.len() - 1)].max(0))
}

/// 校验阶段配置的合法性（配置写错会让人卡死，故必须有校验）。
pub fn validate_stage(
    duration_days: i32,
    remedy_days: i32,
    max_extensions: i32,
) -> DomainResult<()> {
    if duration_days <= 0 {
        return Err(DomainError::Validation(
            "阶段时限必须为正数天".into(),
        ));
    }
    if remedy_days < 0 {
        return Err(DomainError::Validation("补救期不能为负".into()));
    }
    if remedy_days > duration_days * 3 {
        return Err(DomainError::Validation(
            "补救期超过阶段时限 3 倍，多半是配错了".into(),
        ));
    }
    if max_extensions < 0 {
        return Err(DomainError::Validation("延期次数不能为负".into()));
    }
    Ok(())
}

#[cfg(test)]
mod tests;
