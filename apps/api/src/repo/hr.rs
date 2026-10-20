//! 「我的 H&R」读口的 SQL 居所（消费方 `gaps_http/hr_mine.rs`）。
//!
//! 2026-10-17 下沉：赦免三端点早已把 sqlx 计数压在基线上，`hr.rs` 又
//! 因新增分页/状态筛选越过 300 行门禁——把列表这一段搬来 repo，
//! `hr.rs` 只留赦免，`hr_mine.rs` 只留协议适配。

use super::Repo;
use crate::errors::{DomainError, DomainResult};

/// 一行 H&R 快照。`n_*` 四列是**不受状态筛选影响**的全量计数
/// （KPI 条要跨状态给数，不能拿当页行数冒充）。
#[derive(sqlx::FromRow, serde::Serialize)]
pub struct HrRow {
    pub torrent_id: i64,
    pub torrent_name: String,
    pub required_seconds: i32,
    pub seeded_seconds: i32,
    pub deadline: chrono::DateTime<chrono::Utc>,
    pub status: String,
    /// 真实下载完成时刻：取 `snatches.completed_at`。
    /// ⚠️ 不要用 `hr_snapshots.created_at` 代替——那是快照建行时刻，
    /// 而 `deadline` 本就是 completed_at + 宽限期，三者不是一回事。
    #[sqlx(default)]
    pub downloaded_at: Option<chrono::DateTime<chrono::Utc>>,
    /// 当页筛选条件下的总行数（窗口函数带的，不额外打一次 count）
    #[sqlx(default)]
    pub total: i64,
    #[sqlx(default)]
    pub n_open: i64,
    #[sqlx(default)]
    pub n_satisfied: i64,
    #[sqlx(default)]
    pub n_violated: i64,
    #[sqlx(default)]
    pub n_pardoned: i64,
}

/// 状态筛选白名单：认不出的值（含拼错）一律回落「不筛」，
/// 不拿 400 打断这一页的浏览。
pub fn status_filter(raw: Option<&str>) -> Option<&'static str> {
    match raw.map(str::trim) {
        Some("open") => Some("open"),
        Some("satisfied") => Some("satisfied"),
        Some("violated") => Some("violated"),
        Some("pardoned") => Some("pardoned"),
        _ => None,
    }
}

pub async fn my_rows(
    repo: &Repo,
    uid: i64,
    status: Option<&str>,
    limit: i64,
    offset: i64,
) -> DomainResult<Vec<HrRow>> {
    let rows = sqlx::query_as::<_, HrRow>(
        "SELECT h.torrent_id, t.name AS torrent_name, h.required_seconds, \
                h.seeded_seconds, h.deadline, h.status, \
                s.completed_at AS downloaded_at, \
                count(*) OVER() AS total, \
                (SELECT count(*) FROM hr_snapshots h2 \
                  WHERE h2.user_id = $1 AND h2.status = 'open') AS n_open, \
                (SELECT count(*) FROM hr_snapshots h3 \
                  WHERE h3.user_id = $1 \
                    AND h3.status = 'satisfied') AS n_satisfied, \
                (SELECT count(*) FROM hr_snapshots h4 \
                  WHERE h4.user_id = $1 \
                    AND h4.status = 'violated') AS n_violated, \
                (SELECT count(*) FROM hr_snapshots h5 \
                  WHERE h5.user_id = $1 \
                    AND h5.status = 'pardoned') AS n_pardoned \
         FROM hr_snapshots h \
         JOIN torrents t ON t.id = h.torrent_id \
         LEFT JOIN snatches s ON s.user_id = h.user_id \
              AND s.torrent_id = h.torrent_id \
         WHERE h.user_id = $1 \
           AND ($2::text IS NULL OR h.status = $2) \
         ORDER BY h.deadline LIMIT $3 OFFSET $4",
    )
    .bind(uid)
    .bind(status)
    .bind(limit)
    .bind(offset)
    .fetch_all(&repo.db)
    .await
    .map_err(|e| DomainError::Internal(e.into()))?;
    Ok(rows)
}

/// H&R 警告阶梯落在该用户身上的处置位：(禁发新种, 禁发邀请)。
///
/// 读 worker 写入的**当前态**而不是在 API 现算档位：到期恢复
/// （hr_warn_ladder::restore_due）清位后这里自然放行，两边不会各算一套。
/// 无行 = 无警告 = 全 false。总开关关闭时也回 false——否则关掉阶梯后，
/// 上次运行留下的位会继续拦人，「回到旧单档行为」就是假的。
pub async fn warn_blocks(repo: &Repo, uid: i64) -> DomainResult<(bool, bool)> {
    let row: Option<(bool, bool)> = sqlx::query_as(
        "SELECT w.block_upload, w.block_invite \
         FROM hr_warn_states w \
         WHERE w.user_id = $1 \
           AND COALESCE((SELECT value FROM site_settings \
                         WHERE name = 'hr_warn_enable'), '1') \
               NOT IN ('0', 'no')",
    )
    .bind(uid)
    .fetch_optional(&repo.db)
    .await
    .map_err(|e| DomainError::Internal(e.into()))?;
    Ok(row.unwrap_or((false, false)))
}

/// 站点缺省做种时限（小时）。读失败回 48——与 worker 建快照时的
/// COALESCE 缺省一致，不能因为一次设置读不到就把整页打回 500。
pub async fn hours_setting(repo: &Repo) -> i32 {
    sqlx::query_scalar(
        "SELECT COALESCE(NULLIF((SELECT value FROM site_settings \
         WHERE name = 'hr_hours'), '')::int, 48)",
    )
    .fetch_one(&repo.db)
    .await
    .unwrap_or(48)
    .clamp(1, 24 * 365)
}
