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
    let stage: Option<(i32, i32, i32, String, i32, i32, serde_json::Value)> =
        sqlx::query_as(
            "SELECT seq, duration_days, remedy_days, fail_action, reward, \
                    max_extensions, metric \
             FROM exam_stages WHERE task_id = $1 AND seq = $2",
        )
        .bind(c.task_id)
        .bind(c.exam_stage_seq)
        .fetch_optional(db)
        .await?;
    let Some((seq, dur, remedy, action_s, _reward, _ext, _metric)) = stage
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

    let verdict = super::exam_fair::judge_stage(
        &super::exam_fair::StageInput {
            seq,
            is_last,
            duration_days: dur,
            remedy_days: remedy,
            fail_action: super::exam_fair::FailAction::parse(&action_s),
            elapsed_days,
            met,
        },
    );
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
            remedy_until_days,
            ..
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
            apply_fail_action(db, c, action).await?;
            settle_fail(db, c).await?;
            Ok(Some(1))
        }
    }
}

fn now_minus(t: chrono::DateTime<chrono::Utc>) -> chrono::Duration {
    chrono::Utc::now() - t
}

/// 按 fail_action 处置（0381）。
///
/// 列口径对齐 `admin_http/user_status.rs`：`Restrict` → `download_enabled
/// = FALSE`（禁下载不封号），`Ban` → `status = 2`（0正常/1禁言/2封号）。
/// 只改既有列，后台人工入口与考核自动入口写同一处，不会出现两套真相。
///
/// ⚠️ tracker 的 passkey 缓存在 api 侧 60s，worker 这里改完无法立即让
/// 失效。故建议站长优先用 `restrict`（人工复核），真要 `ban` 也应确认
/// 失效窗口可接受——这条路是伤害性操作，不该默认打开。
pub(super) async fn apply_fail_action(
    db: &PgPool,
    c: &OpenClaim,
    action: super::exam_fair::FailAction,
) -> anyhow::Result<()> {
    use super::exam_fair::FailAction;
    match action {
        FailAction::None => Ok(()),
        FailAction::Restrict => {
            sqlx::query(
                "UPDATE users SET download_enabled = FALSE \\
                 WHERE id = $1 AND download_enabled = TRUE",
            )
            .bind(c.user_id)
            .execute(db)
            .await?;
            Ok(())
        }
        FailAction::Ban => {
            sqlx::query(
                "UPDATE users SET status = 2 \
                 WHERE id = $1 AND status < 2",
            )
            .bind(c.user_id)
            .execute(db)
            .await?;
            Ok(())
        }
    }
}
