//! 任务结算（0051）：认领结算主循环与达标/失败结算。
//! 口径（对齐参考站 UserTaskRecord 的 base + delta）：
//!   upload_delta      = users.uploaded - base_uploaded
//!   download_delta    = users.downloaded - base_downloaded（仅用于 Master 系列等级任务的总流量口径）
//!   seed_seconds_delta= sum(snatches.seeded_seconds) - base_seed_seconds（做种时长）
//!   uploads           = 发布种子数 - base_uploads（领取后新发布）
//! metric JSON 里出现的键参与判定：全部达标 → 完成；任务到期/超时限 → 失败。
//! 注：等级系列任务（Master/Ultimate…）metric 是累计门槛而非增量，
//!     此类任务 base 记 0 且用 users 现值直接比对（tier 非空视为累计口径）。

use super::reward::{settle_complete, settle_fail};
use super::stages::{stage_advance, stage_deadline};
use sqlx::PgPool;

#[derive(serde::Deserialize, Default)]
pub(super) struct TaskMetric {
    #[serde(default)]
    pub(crate) upload_delta: Option<i64>,
    #[serde(default)]
    pub(crate) download_delta: Option<i64>,
    /// 做种积分：1 积分 = 1 小时做种（与 class_rules.min_seed_hours 同源）
    #[serde(default)]
    pub(crate) seed_points_delta: Option<i64>,
    /// 做种时长增量（秒），无折算歧义；tier 累计口径下为绝对值
    #[serde(default)]
    pub(crate) seed_seconds_delta: Option<i64>,
    #[serde(default)]
    pub(crate) uploads: Option<i64>,
    #[serde(default)]
    pub(crate) subtitles: Option<i64>,
}

#[derive(sqlx::FromRow)]
pub(super) struct OpenClaim {
    pub(super) id: i64,
    pub(super) task_id: i64,
    pub(super) user_id: i64,
    pub(super) reward: i64,
    pub(super) penalty: i64,
    pub(super) duration_days: i32,
    pub(crate) tier: Option<String>,
    /// 0093：任务类型（task/onboard/periodic），用于完成通知区分转正考核文案
    pub(crate) kind: String,
    pub(crate) task_name: String,
    pub(crate) metric: serde_json::Value,
    pub(super) claimed_at: chrono::DateTime<chrono::Utc>,
    pub(super) base_uploaded: i64,
    pub(super) base_seed_seconds: i64,
    pub(super) base_uploads: i64,
    // 0381 多阶段：当前阶段（无阶段配置时恒 1，行为与迁移前一致）。
    // 四列在 0381 里都是 NOT NULL / DEFAULT，值永不为 NULL，故不需要
    // #[sqlx(default)]（FromRow derive 也不支持 default = "fn" 写法）。
    pub(super) exam_stage_seq: i32,
    /// 本阶段开始时刻（NULL = 用认领时刻兜底）
    pub(super) exam_stage_started_at: Option<chrono::DateTime<chrono::Utc>>,
    /// 补救期截止（NULL = 未进补救）
    pub(super) exam_remedy_until: Option<chrono::DateTime<chrono::Utc>>,
    /// 已用延期次数
    pub(super) exam_extensions: i32,
}

/// 结算一次所有进行中的认领。worker 分钟级调用（幂等：状态推进用条件更新 + 发奖幂等键）。
pub async fn task_settle(db: &PgPool) -> anyhow::Result<u64> {
    let claims: Vec<OpenClaim> = sqlx::query_as(
        "SELECT c.id, c.task_id, c.user_id, t.reward, t.penalty, t.duration_days, t.tier, \
                t.kind, t.name AS task_name, t.metric, c.claimed_at, \
                c.base_uploaded, c.base_seed_seconds, c.base_uploads, \
                c.exam_stage_seq, c.exam_stage_started_at, \
                c.exam_remedy_until, c.exam_extensions \
         FROM task_claims c JOIN tasks t ON t.id = c.task_id \
         WHERE c.status = 0 AND c.exempted_at IS NULL LIMIT $1",
    )
    .bind(crate::jobs::JOB_BATCH)
    .fetch_all(db)
    .await?;

    let mut done = 0u64;
    for c in claims {
        let metric: TaskMetric =
            serde_json::from_value(c.metric.clone()).unwrap_or_default();
        let now = chrono::Utc::now();

        // 用户当前指标
        let cur: Option<(i64, i64)> = sqlx::query_as(
            "SELECT uploaded, downloaded FROM users WHERE id = $1",
        )
        .bind(c.user_id)
        .fetch_optional(db)
        .await?;
        let Some((uploaded, downloaded)) = cur else {
            continue;
        }; // 用户已删，跳过

        // sum(bigint)→numeric 同坑（站点统计三连之一）：补 ::bigint 防 i64 解码
        // 失败被 unwrap_or(0) 吞掉——任务结算的做种时长指标会恒 0
        let seed_seconds: i64 = sqlx::query_scalar(
            "SELECT COALESCE(sum(seeded_seconds), 0)::bigint FROM snatches \
             WHERE user_id = $1",
        )
        .bind(c.user_id)
        .fetch_one(db)
        .await
        .unwrap_or_else(|e| {
            tracing::warn!(%e, "任务结算 seeded_seconds 汇总失败");
            0
        });
        let uploads_now: i64 = sqlx::query_scalar(
            "SELECT count(*) FROM torrents WHERE owner_id = $1 AND \
             approval_status = 1",
        )
        .bind(c.user_id)
        .fetch_one(db)
        .await
        .unwrap_or(0);
        let subtitles_now: i64 = sqlx::query_scalar(
            "SELECT count(*) FROM subtitles WHERE user_id = $1",
        )
        .bind(c.user_id)
        .fetch_one(db)
        .await
        .unwrap_or(0);

        // 累计口径（tier 任务）：base 视为 0，用现值直接比
        let cumulative = c.tier.is_some();
        let (up_d, seed_d, uploads_d) = if cumulative {
            (uploaded, seed_seconds, uploads_now)
        } else {
            (
                uploaded - c.base_uploaded,
                seed_seconds - c.base_seed_seconds,
                uploads_now - c.base_uploads,
            )
        };

        // 达标判定：metric 里出现的键全部满足
        let mut met = true;
        let mut has_target = false;
        if let Some(v) = metric.upload_delta {
            has_target = true;
            met &= up_d >= v;
        }
        if let Some(v) = metric.download_delta {
            has_target = true;
            met &= downloaded >= v;
        }
        if let Some(v) = metric.seed_points_delta {
            has_target = true;
            // 口径厘清（P1-6）：v 的单位是「做种积分」，1 积分 = 1 小时做种时长，
            // 与 class_rules.min_seed_hours 同源；判定 = 累计做种秒 ≥ v×3600。
            // 旧公式 v*3600/100（v×36 秒）无站内依据：站内 seed_points = 在做种数×100
            // （http.rs 口径）与时长无关，旧折算既不对应站内积分也不对应时长，废弃。
            met &= seed_d >= v.saturating_mul(3600);
        }
        if let Some(v) = metric.seed_seconds_delta {
            has_target = true;
            met &= seed_d >= v; // 秒口径，1 小时 = 3600，无折算歧义
        }
        if let Some(v) = metric.uploads {
            has_target = true;
            met &= uploads_d >= v;
        }
        if let Some(v) = metric.subtitles {
            has_target = true;
            met &= subtitles_now >= v;
        }

        // 0381：多阶段考核走状态机判定（domain::exam），单段/未配置阶段
        // 的 task 行为与迁移前逐位一致（remedy_days=0 + seq=1）。
        if let Some(adv) = stage_advance(db, &c, met && has_target)
            .await?
        {
            done += adv;
            continue;
        }
        if met && has_target {
            if settle_complete(db, &c).await? {
                done += 1;
            }
        } else {
            let deadline = stage_deadline(&c);
            let ended = now > deadline || now > task_end(db, c.task_id).await;
            if ended {
                settle_fail(db, &c).await?;
                done += 1;
            }
        }
    }
    Ok(done)
}

async fn task_end(db: &PgPool, task_id: i64) -> chrono::DateTime<chrono::Utc> {
    sqlx::query_scalar("SELECT ends_at FROM tasks WHERE id = $1")
        .bind(task_id)
        .fetch_one(db)
        .await
        .unwrap_or(chrono::DateTime::<chrono::Utc>::MAX_UTC)
}
