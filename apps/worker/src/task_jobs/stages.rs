//! 考核多阶段推进与补救期（0381，PT Studio FR-EXAM 对标）。
//!
//! 自 settle.rs 抽出（守 300 行门禁）。判定矩阵在 `api::domain::exam`
//! 纯函数层，本文件只负责取配置、算天数、落库。
//!
//! 【向后兼容】exam_stages 里没配该 task 的阶段时 `stage_advance` 返回
//! `None`，调用方走旧的单段结算路径——行为与迁移前逐位一致。

use sqlx::PgPool;

use super::reward::{settle_complete, settle_fail};
use super::settle::OpenClaim;

/// 本阶段的截止时刻：多阶段用阶段起点 + 阶段时限，否则用认领时刻。
pub(super) fn stage_deadline(c: &OpenClaim) -> chrono::DateTime<chrono::Utc> {
    let start = c.exam_stage_started_at.unwrap_or(c.claimed_at);
    start + chrono::Duration::days(c.duration_days.max(1) as i64)
}

/// 该认领是否走多阶段路径（有阶段配置即走）。
///
/// 返回 Some(本轮推进了几条) 表示本轮由阶段状态机处理完毕，
/// 调用方应 `continue` 跳过旧的单段结算。
pub(super) async fn stage_advance(
    db: &PgPool,
    c: &OpenClaim,
    met: bool,
) -> anyhow::Result<Option<u64>> {
    let stage: Option<(
        i32,
        i32,
        i32,
        String,
        i32,
        i32,
        serde_json::Value,
        serde_json::Value,
    )> = sqlx::query_as(
        "SELECT seq, duration_days, remedy_days, fail_action, reward, \
                    max_extensions, metric, fail_penalty \
             FROM exam_stages WHERE task_id = $1 AND seq = $2",
    )
    .bind(c.task_id)
    .bind(c.exam_stage_seq)
    .fetch_optional(db)
    .await?;
    let Some((
        seq,
        dur,
        remedy,
        action_s,
        _reward,
        _ext,
        _metric,
        fail_penalty,
    )) = stage
    else {
        // 没配阶段 → 旧路径。exam_stage_seq 也可能是 1（默认值）。
        return Ok(None);
    };
    let is_last: bool = !sqlx::query_scalar::<_, bool>(
        "SELECT EXISTS(SELECT 1 FROM exam_stages \
         WHERE task_id = $1 AND seq > $2)",
    )
    .bind(c.task_id)
    .bind(seq)
    .fetch_one(db)
    .await?;

    // 补救期不需要在这里特判：`judge_stage` 收的是「本阶段已过天数」，
    // 内部按 duration_days + remedy_days 一次算完逾期与宽限余量。
    // 早先这里另起了一段「用补救截止重算逾期」的分支，结果是恒等变换
    // ——写的时候以为要换基准，其实状态机已经处理了。
    let started = c.exam_stage_started_at.unwrap_or(c.claimed_at);
    let elapsed_days = (now_minus(started)).num_days().max(0) as i32;

    let verdict =
        super::exam_fair::judge_stage(&super::exam_fair::StageInput {
            seq,
            is_last,
            duration_days: dur,
            remedy_days: remedy,
            fail_action: super::exam_fair::FailAction::parse(&action_s),
            elapsed_days,
            met,
        });
    let prog = super::exam_fair::progress(&verdict, seq);
    match prog {
        super::exam_fair::Progress::None => Ok(None),
        super::exam_fair::Progress::Advance { next_seq, .. } => {
            let n = sqlx::query(
                "UPDATE task_claims \
                 SET exam_stage_seq = $2, exam_stage_started_at = now(), \
                     exam_remedy_until = NULL \
                 WHERE id = $1 AND status = 0",
            )
            .bind(c.id)
            .bind(next_seq)
            .execute(db)
            .await?
            .rows_affected();
            Ok(Some(n))
        }
        super::exam_fair::Progress::ExamPassed => {
            if settle_complete(db, c).await? {
                Ok(Some(1))
            } else {
                Ok(None)
            }
        }
        super::exam_fair::Progress::EnterRemedy {
            remedy_until_days, ..
        } => {
            let n = sqlx::query(
                "UPDATE task_claims SET exam_remedy_until = \
                 now() + ($2 || ' days')::interval \
                 WHERE id = $1 AND status = 0",
            )
            .bind(c.id)
            .bind(remedy_until_days.to_string())
            .execute(db)
            .await?
            .rows_affected();
            Ok(Some(n))
        }
        super::exam_fair::Progress::Failed { action } => {
            // 0397：fail_penalty 菜单优先；空则回退 fail_action 三态
            // （0381 兼容——已有站点的处置语义零变化）
            let items = match fail_penalty.as_array() {
                Some(a) if !a.is_empty() => {
                    super::penalty::parse_penalty(fail_penalty)
                }
                _ => vec![],
            };
            let mut items = items;
            // 三态映射（fail_action → 菜单项）：处置细节统一走 penalty
            // 模块，本文件不再各写一份 UPDATE
            if items.is_empty() {
                match action {
                    super::exam_fair::FailAction::Restrict => {
                        items = vec![penalty_item(
                            super::penalty::KIND_RESTRICT_DOWNLOAD,
                        )];
                    }
                    super::exam_fair::FailAction::Ban => {
                        items = vec![penalty_item(super::penalty::KIND_BAN)];
                    }
                    super::exam_fair::FailAction::None => {}
                }
            }
            // 0397：处置结果随站内信告知用户（黑箱处置＝白处置）
            let notes =
                super::penalty::apply_penalties(db, c, seq, &items).await?;
            settle_fail(db, c, &notes).await?;
            Ok(Some(1))
        }
    }
}

/// 无参数菜单项（三态回退用）
fn penalty_item(kind: &str) -> super::penalty::PenaltyItem {
    super::penalty::PenaltyItem {
        kind: kind.into(),
        amount: None,
        target: None,
        pages: None,
    }
}

fn now_minus(t: chrono::DateTime<chrono::Utc>) -> chrono::Duration {
    chrono::Utc::now() - t
}
