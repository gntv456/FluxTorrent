//! 勋章自动成就的读面（FRAME §2 H 层不下沉 sqlx）。
//!
//! 消费方：`community_http/medal_progress.rs`（用户看「还差多少」）。
//! 判定在 `domain::medal`（progress_ratio / progress_text），本文件只取数。

use super::Repo;
use crate::errors::{DomainError, DomainResult};

/// 一条自动成就 + 当前进度。
#[derive(Debug, Clone, sqlx::FromRow, serde::Serialize)]
pub struct ProgressRow {
    pub id: i64,
    pub name: String,
    pub description: Option<String>,
    pub asset_ref: Option<String>,
    /// 达成阈值
    pub auto_target: i64,
    /// 判定口径：uploaded / seed_hours / uploads / registered_days /
    /// 或 event 类的事件名（seed / upload / comment …）
    pub auto_rule: Option<String>,
    /// 当前累计
    pub current: i64,
    /// 是否已授予
    pub awarded: bool,
    pub awarded_at: Option<chrono::DateTime<chrono::Utc>>,
    pub wearing: bool,
    /// 进度条是否可见（有的成就刻意不露进度，避免「还差 3 小时就到手」
    /// 这种压力）
    pub progress_visible: bool,
    pub rarity: Option<String>,
    pub category_id: i32,
    pub sort_order: i32,
}

/// 我的自动成就进度（含已达成的）。
pub async fn my_progress(
    repo: &Repo,
    user_id: i64,
) -> DomainResult<Vec<ProgressRow>> {
    Ok(sqlx::query_as(
        "SELECT m.id, m.name, m.description, m.asset_ref, \
                COALESCE(m.auto_target, 0) AS auto_target, \
                m.auto_rule, \
                COALESCE(p.current, 0) AS current, \
                (um.medal_id IS NOT NULL) AS awarded, \
                p.awarded_at, \
                COALESCE(um.wearing, FALSE) AS wearing, \
                m.progress_visible, m.rarity, \
                COALESCE(m.category_id, 0) AS category_id, \
                COALESCE(m.sort_order, 0) AS sort_order \
         FROM medals m \
         LEFT JOIN medal_progress p \
           ON p.medal_id = m.id AND p.user_id = $1 \
         LEFT JOIN user_medals um \
           ON um.medal_id = m.id AND um.user_id = $1 \
         WHERE m.auto_mode IS NOT NULL \
         ORDER BY m.sort_order, m.id",
    )
    .bind(user_id)
    .fetch_all(&repo.db)
    .await
    .map_err(|e| DomainError::Internal(e.into()))?)
}

/// 我的累计统计（threshold 类成就的「当前值」来源）。
///
/// 口径与 worker 的 `medal_achv::eval::threshold_value` 同源——两边
/// 各算一次必然漂移，故这里刻意复用同一组列。
pub async fn my_stats(repo: &Repo, user_id: i64) -> DomainResult<Stats> {
    let uploaded: i64 = sqlx::query_scalar(
        "SELECT COALESCE(uploaded, 0)::bigint FROM users WHERE id = $1",
    )
    .bind(user_id)
    .fetch_optional(&repo.db)
    .await
    .map_err(|e| DomainError::Internal(e.into()))?
    .unwrap_or(0);
    // sum(bigint) → numeric 的同坑：必须 ::bigint，否则 i64 解码失败恒 0
    let seed_hours: i64 = sqlx::query_scalar(
        "SELECT COALESCE(sum(seeded_seconds), 0)::bigint / 3600 \
         FROM snatches WHERE user_id = $1",
    )
    .bind(user_id)
    .fetch_one(&repo.db)
    .await
    .map_err(|e| DomainError::Internal(e.into()))?;
    let uploads: i64 = sqlx::query_scalar(
        "SELECT count(*)::bigint FROM torrents \
         WHERE owner_id = $1 AND approval_status = 1",
    )
    .bind(user_id)
    .fetch_one(&repo.db)
    .await
    .map_err(|e| DomainError::Internal(e.into()))?;
    let registered_days: i64 = sqlx::query_scalar(
        "SELECT GREATEST(0, EXTRACT(DAY FROM now() - created_at))::bigint \
         FROM users WHERE id = $1",
    )
    .bind(user_id)
    .fetch_one(&repo.db)
    .await
    .map_err(|e| DomainError::Internal(e.into()))?;
    Ok(Stats {
        uploaded,
        seed_hours,
        uploads,
        registered_days,
    })
}

/// 四项累计统计。
#[derive(Debug, Clone, Copy)]
pub struct Stats {
    pub uploaded: i64,
    pub seed_hours: i64,
    pub uploads: i64,
    pub registered_days: i64,
}

impl Stats {
    /// 按规则名取当前值。
    ///
    /// **未识别的规则名返回 `None`**（而不是 0）——调用方据此判断
    /// 「这条成就的口径没接上」，可以记日志提醒站长补规则。
    /// 若返回 0，用户会看到一个永远 0% 的进度条，且没有任何线索。
    pub fn of(&self, rule: &str) -> Option<i64> {
        match rule {
            "uploaded" => Some(self.uploaded),
            "seed_hours" => Some(self.seed_hours),
            "uploads" => Some(self.uploads),
            "registered_days" => Some(self.registered_days),
            _ => None,
        }
    }
}
