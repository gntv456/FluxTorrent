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
use sqlx::PgPool;

#[derive(serde::Deserialize, Default)]
pub(crate) struct TaskMetric {
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
pub(crate) struct OpenClaim {
    pub(crate) id: i64,
    pub(crate) task_id: i64,
    pub(crate) user_id: i64,
    pub(crate) reward: i64,
    pub(crate) penalty: i64,
    pub(crate) duration_days: i32,
    pub(crate) tier: Option<String>,
    /// 0093：任务类型（task/onboard/periodic），用于完成通知区分转正考核文案
    #[sqlx(default)]
    pub(crate) kind: String,
    #[sqlx(default)]
    pub(crate) task_name: String,
    pub(crate) metric: serde_json::Value,
    pub(crate) claimed_at: chrono::DateTime<chrono::Utc>,
    pub(crate) base_uploaded: i64,
    pub(crate) base_seed_seconds: i64,
    pub(crate) base_uploads: i64,
}

/// 结算一次所有进行中的认领。worker 分钟级调用（幂等：状态推进用条件更新 + 发奖幂等键）。
pub async fn task_settle(db: &PgPool) -> anyhow::Result<u64> {
    let claims: Vec<OpenClaim> = sqlx::query_as(
        "SELECT c.id, c.task_id, c.user_id, t.reward, t.penalty, t.duration_days, t.tier, \
                t.kind, t.name AS task_name, t.metric, c.claimed_at, \
                c.base_uploaded, c.base_seed_seconds, c.base_uploads \
         FROM task_claims c JOIN tasks t ON t.id = c.task_id \
         WHERE c.status = 0 AND c.exempted_at IS NULL LIMIT 500",
    )
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

        let seed_seconds: i64 = sqlx::query_scalar(
            "SELECT COALESCE(sum(seeded_seconds), 0) FROM snatches WHERE user_id = $1",
        )
        .bind(c.user_id)
        .fetch_one(db)
        .await
        .unwrap_or(0);
        let uploads_now: i64 = sqlx::query_scalar(
            "SELECT count(*) FROM torrents WHERE owner_id = $1 AND approval_status = 1",
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

        let deadline = c.claimed_at
            + chrono::Duration::days(c.duration_days.max(1) as i64);
        if met && has_target {
            if settle_complete(db, &c).await? {
                done += 1;
            }
        } else if now > deadline || now > task_end(db, c.task_id).await {
            settle_fail(db, &c).await?;
            done += 1;
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
