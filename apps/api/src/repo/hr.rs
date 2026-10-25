//! 「我的 H&R」读口的 SQL 居所（消费方 `gaps_http/hr_mine.rs`）。
//!
//! 2026-10-17 下沉：赦免三端点早已把 sqlx 计数压在基线上，`hr.rs` 又
//! 因新增分页/状态筛选越过 300 行门禁——把列表这一段搬来 repo，
//! `hr.rs` 只留赦免，`hr_mine.rs` 只留协议适配。

use super::Repo;
use crate::errors::{DomainError, DomainResult};
use sqlx::PgPool;

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
pub async fn hours_setting(db: &PgPool) -> i32 {
    sqlx::query_scalar(
        "SELECT COALESCE(NULLIF((SELECT value FROM site_settings \
         WHERE name = 'hr_hours'), '')::int, 48)",
    )
    .fetch_one(db)
    .await
    .unwrap_or(48)
    .clamp(1, 24 * 365)
}

/// 站点缺省期限（天）。与 worker `jobs/hr.rs` 的 `hr_days` COALESCE 同源；
/// 此前展示侧（详情页与 `/me/hr`）把它写死成 14，站长改设定不跟动。
pub async fn days_setting(db: &PgPool) -> i32 {
    sqlx::query_scalar(
        "SELECT COALESCE(NULLIF((SELECT value FROM site_settings \
         WHERE name = 'hr_days'), '')::int, 14)",
    )
    .fetch_one(db)
    .await
    .unwrap_or(14)
    .clamp(1, 365)
}

/// 某会员对某颗种子的**生效** H&R 义务：`(是否追责, 期限天, 需做种小时)`。
///
/// 口径链（与 worker 建快照逐字同源，缺一层界面就会承诺得比追责轻）：
///   1. 开关取种子 `hr_policy` 的 `enabled`，兼容历史写入的 `on` 键，缺省追责；
///   2. 基线取种子 policy 的 `days`/`seed_hours`，缺省回落站点设定；
///   3. 按档位折扣走 `hr_reduced_days` / `hr_reduced_seed_hours`
///      （第三参传 0＝尚未开始做种，减免自然为 0）。
///
/// 实测现网：strict 档绑 class 1（普通会员），基线 48h/14 天被执法成
/// **72h/11 天**，而详情页对所有人写「14 天内需做种 48 小时」——按界面
/// 承诺做种的人攒够 48h 仍会吃违规、累计到阈值直接停下载权。
pub async fn effective(
    db: &PgPool,
    torrent_id: i64,
    class_id: i32,
) -> DomainResult<(bool, i32, i32)> {
    let (d_def, h_def) = (days_setting(db).await, hours_setting(db).await);
    let row = sqlx::query_as::<_, (bool, i32, i32)>(
        "SELECT COALESCE((t.hr_policy->>'enabled') IN ('true', '1'), \
                          (t.hr_policy->>'on') IN ('true', '1'), TRUE), \
         hr_reduced_days( \
             COALESCE((t.hr_policy->>'days')::int, $3::int), $2::int), \
         hr_reduced_seed_hours( \
             COALESCE((t.hr_policy->>'seed_hours')::int, $4::int), \
             $2::int, 0) \
         FROM torrents t WHERE t.id = $1",
    )
    .bind(torrent_id)
    .bind(class_id)
    .bind(d_def)
    .bind(h_def)
    .fetch_optional(db)
    .await
    .map_err(|e| DomainError::Internal(e.into()))?;
    // 种子不存在/读不到时按「追责 + 站点缺省」回，绝不让详情页打回 500
    Ok(row.unwrap_or((true, d_def, h_def)))
}

/// 把详情主行的 `hr_policy` 换成**本人视角的生效义务**：前端读到的就是
/// 真会被执法追究的那个值（`get_torrent` 一处覆盖，详情/聚合/OpenAPI
/// 三个出口一起对上；列表端点不走，免得每颗种多两跳查询）。
/// viewer 缺失（OpenAPI 演示口）按未折扣档算，仍比前端写死 14/48 贴近执法。
pub async fn override_policy_effective(
    db: &PgPool,
    row: &mut crate::torrents::TorrentRow,
    viewer: Option<i64>,
) -> DomainResult<()> {
    let class: i32 = match viewer {
        Some(v) => {
            sqlx::query_scalar("SELECT class_id FROM users WHERE id = $1")
                .bind(v)
                .fetch_optional(db)
                .await
                .map_err(|e| DomainError::Internal(e.into()))?
                .unwrap_or(0)
        }
        None => 0,
    };
    let (on, days, hours) = effective(db, row.id, class).await?;
    row.hr_policy = Some(serde_json::json!({
        "enabled": on,
        "days": days,
        "seed_hours": hours,
    }));
    Ok(())
}
