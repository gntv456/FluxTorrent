//! 勋章自动成就（0382 worker 侧，三条路径共用一份判定）。
//!
//! 1. **event**：事件流累加 → 达标即授予（`event.rs`）
//! 2. **threshold**：周期性整体重算 → 达标即授予（`eval.rs`）
//! 3. **backfill**：新建成就时对存量用户扫一轮（`backfill.rs`）
//!
//! 判定矩阵在 `api::domain::medal`（include! 复用同一份源文件），
//! 本 crate 只负责取数与落库。

mod backfill;
mod eval;
mod event;

pub(super) use backfill::backfill;
pub(crate) use backfill::medal_backfill;
pub(super) use eval::threshold_sweep;
pub(crate) use eval::prog;
// `record_event` 是给业务侧（发种/评论等处）调的入口，当前站内尚未
// 接线——接线时直接 `use crate::jobs::medal_achv::record_event`。
#[allow(unused_imports)]
pub(super) use event::record_event;

/// 判定矩阵的真身在 api（`api/src/domain/medal`），worker 用 include!
/// 复用同一份源文件——两 crate 各写一份迟早漂移，而漂移的后果是
/// 「api 算达标、worker 不发」或反过来。
#[path = "../../../../api/src/domain/medal/mod.rs"]
mod api_domain;
// 判定矩阵（真身在 api/src/domain/medal）整包转出：本模块的三个子文件
// 统一用 `super::api_domain as md` 引用，避免各自写 #[path]。
#[allow(unused_imports)]
pub(crate) use api_domain::*;

/// 一轮扫描的批大小（够小则单轮秒级，够大则减少轮次）。
pub(super) const BATCH: i64 = 500;
/// 单轮最多扫多少批（防止一次 tick 跑太久）。
pub(super) const MAX_BATCHES: i32 = 20;

/// 一条成就规则（从 medals 读出的最小集）。
#[derive(sqlx::FromRow)]
pub(super) struct RuleRow {
    pub(super) id: i64,
    pub(super) name: String,
    pub(super) auto_rule: Option<String>,
    pub(super) auto_mode: Option<String>,
    pub(super) auto_target: Option<i64>,
    pub(super) auto_period: Option<String>,
}

/// 读出全部生效的自动成就（按 mode 过滤）。
pub(super) async fn rules_of(
    db: &sqlx::PgPool,
    mode: Option<&str>,
) -> anyhow::Result<Vec<RuleRow>> {
    let sql = "SELECT id, name, auto_rule, auto_mode, auto_target, \
               auto_period FROM medals \
               WHERE active = TRUE AND auto_mode IS NOT NULL \
                 AND ($1::text IS NULL OR auto_mode = $1)";
    Ok(sqlx::query_as(sql)
        .bind(mode)
        .fetch_all(db)
        .await?)
}

/// 是否已授予（幂等判据之一）。
pub(super) async fn awarded(
    db: &sqlx::PgPool,
    user_id: i64,
    medal_id: i64,
) -> bool {
    sqlx::query_scalar(
        "SELECT EXISTS(SELECT 1 FROM user_medals \
         WHERE user_id = $1 AND medal_id = $2)",
    )
    .bind(user_id)
    .bind(medal_id)
    .fetch_one(db)
    .await
    .unwrap_or(false)
}

/// 读进度（无行视作 0）。
pub(super) async fn progress_of(
    db: &sqlx::PgPool,
    user_id: i64,
    medal_id: i64,
) -> i64 {
    sqlx::query_scalar(
        "SELECT COALESCE(current, 0)::bigint FROM medal_progress \
         WHERE user_id = $1 AND medal_id = $2",
    )
    .bind(user_id)
    .bind(medal_id)
    .fetch_one(db)
    .await
    .unwrap_or(0)
}

/// 落进度（只增不减：防并发下的回退）。
pub(super) async fn save_progress(
    db: &sqlx::PgPool,
    user_id: i64,
    medal_id: i64,
    current: i64,
) -> anyhow::Result<()> {
    sqlx::query(
        "INSERT INTO medal_progress (user_id, medal_id, current) \
         VALUES ($1, $2, $3) \
         ON CONFLICT (user_id, medal_id) DO UPDATE \
         SET current = GREATEST(medal_progress.current, EXCLUDED.current), \
             updated_at = now()",
    )
    .bind(user_id)
    .bind(medal_id)
    .bind(current)
    .execute(db)
    .await?;
    Ok(())
}

/// 授予（幂等三道闸之一：部分唯一索引 + ON CONFLICT + awarded 预检）。
pub(super) async fn award(
    db: &sqlx::PgPool,
    user_id: i64,
    r: &RuleRow,
    source: &str,
) -> anyhow::Result<bool> {
    let n = sqlx::query(
        "INSERT INTO user_medals (user_id, medal_id, source) \
         VALUES ($1, $2, $3) ON CONFLICT DO NOTHING",
    )
    .bind(user_id)
    .bind(r.id)
    .bind(source)
    .execute(db)
    .await?
    .rows_affected();
    if n == 0 {
        return Ok(false);
    }
    sqlx::query(
        "UPDATE medal_progress SET awarded_at = now(), updated_at = now() \
         WHERE user_id = $1 AND medal_id = $2",
    )
    .bind(user_id)
    .bind(r.id)
    .execute(db)
    .await?;
    tracing::info!(user_id, medal = %r.name, source, "勋章自动授予");
    Ok(true)
}

/// 还没拿到这个成就的活用户 id（升序，可续跑）。
pub(super) async fn pending_users(
    db: &sqlx::PgPool,
    medal_id: i64,
    limit: i64,
) -> anyhow::Result<Vec<i64>> {
    Ok(sqlx::query_scalar(
        "SELECT u.id FROM users u WHERE u.status < 2 \
         AND NOT EXISTS (SELECT 1 FROM user_medals m \
                         WHERE m.user_id = u.id AND m.medal_id = $1) \
         ORDER BY u.id LIMIT $2",
    )
    .bind(medal_id)
    .bind(limit)
    .fetch_all(db)
    .await?)
}
