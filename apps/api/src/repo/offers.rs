//! 候选推荐（offers）的全部数据访问（`content_http/offers.rs` 的 SQL
//! 唯一居所）。2026-10-15 下沉：FRAME §2「H 不 import sqlx」。
//!
//! 搬运时逐字保留三处口径（都是踩过坑定下来的，改动请连注释一起看）：
//!   · 投票先占位再扣款：扣款失败要能删掉占位行，否则留下"已投过票"假象
//!   · 转正先抢 offers.promoted，抢到再验种是否在架；抢不到/种已下架都要
//!     **回滚 promoted**（P-317：过期的候选不得撤销审核台的裁决）
//!   · 自审拦截在转正之后做，回滚同样要把 promoted 置回 false

use super::Repo;
use sqlx::FromRow;

/// 候选行（含提名人与种子名，LEFT JOIN 允许两列为空）。
#[derive(FromRow, serde::Serialize)]
pub struct OfferRow {
    pub id: i64,
    pub username: Option<String>,
    pub torrent_id: i64,
    pub torrent_name: Option<String>,
    pub votes: i32,
    pub promoted: bool,
    pub created_at: chrono::DateTime<chrono::Utc>,
}

/// 种是否存在且已过审（提名前置校验；读失败按「否」拒）。
pub async fn torrent_approved(repo: &Repo, torrent_id: i64) -> bool {
    sqlx::query_scalar(
        "SELECT EXISTS(SELECT 1 FROM torrents WHERE id = $1 AND \
         approval_status = 1)",
    )
    .bind(torrent_id)
    .fetch_one(&repo.db)
    .await
    .unwrap_or(false)
}

/// 提名（返回新候选 id）。
pub async fn create_offer(
    repo: &Repo,
    user_id: i64,
    torrent_id: i64,
) -> Result<i64, sqlx::Error> {
    sqlx::query_scalar(
        "INSERT INTO offers (user_id, torrent_id) VALUES ($1, $2) \
         RETURNING id",
    )
    .bind(user_id)
    .bind(torrent_id)
    .fetch_one(&repo.db)
    .await
}

/// 候选项列表（未转正，按票数降序，限 50）。
pub async fn list_offers(repo: &Repo) -> Result<Vec<OfferRow>, sqlx::Error> {
    sqlx::query_as::<_, OfferRow>(
        "SELECT o.id, u.username, o.torrent_id, t.name AS torrent_name, \
         o.votes, o.promoted, o.created_at \
         FROM offers o LEFT JOIN users u ON u.id = o.user_id \
         LEFT JOIN torrents t ON t.id = o.torrent_id \
         WHERE NOT o.promoted ORDER BY o.votes DESC, o.id DESC LIMIT 50",
    )
    .fetch_all(&repo.db)
    .await
}

/// 候选存在且未转正（投票前置校验；读失败按「否」→ NotFound）。
pub async fn voteable(repo: &Repo, offer_id: i64) -> bool {
    sqlx::query_scalar(
        "SELECT EXISTS(SELECT 1 FROM offers WHERE id = $1 AND NOT promoted)",
    )
    .bind(offer_id)
    .fetch_one(&repo.db)
    .await
    .unwrap_or(false)
}

/// 占位投票（原子判重）。返回 false = 已投过。
pub async fn place_vote(
    repo: &Repo,
    offer_id: i64,
    user_id: i64,
) -> Result<bool, sqlx::Error> {
    let r = sqlx::query(
        "INSERT INTO offer_votes (offer_id, user_id, \
         cost) VALUES ($1, $2, 1) ON CONFLICT DO NOTHING",
    )
    .bind(offer_id)
    .bind(user_id)
    .execute(&repo.db)
    .await?;
    Ok(r.rows_affected() > 0)
}

/// 扣款失败时撤掉投票占位（best-effort，失败也不阻断报错返回）。
pub async fn drop_vote(repo: &Repo, offer_id: i64, user_id: i64) {
    let _ = sqlx::query(
        "DELETE FROM offer_votes WHERE offer_id = $1 AND user_id = $2",
    )
    .bind(offer_id)
    .bind(user_id)
    .execute(&repo.db)
    .await;
}

/// 计票 +1（仍限未转正）。返回 false = 已被转正，投票落空。
pub async fn count_vote(
    repo: &Repo,
    offer_id: i64,
) -> Result<bool, sqlx::Error> {
    let r = sqlx::query(
        "UPDATE offers SET votes = votes + 1 WHERE id = $1 AND NOT promoted",
    )
    .bind(offer_id)
    .execute(&repo.db)
    .await?;
    Ok(r.rows_affected() > 0)
}

/// 抢占转正（`NOT promoted` 保证一人一次）。返回对应 torrent_id。
pub async fn claim_promote(
    repo: &Repo,
    offer_id: i64,
) -> Result<Option<i64>, sqlx::Error> {
    sqlx::query_scalar(
        "UPDATE offers SET promoted = true WHERE id = $1 AND NOT \
         promoted RETURNING torrent_id",
    )
    .bind(offer_id)
    .fetch_optional(&repo.db)
    .await
}

/// 自审回滚 / 在架守卫失败回滚：把 promoted 置回 false。
pub async fn unpromote(repo: &Repo, offer_id: i64) {
    let _ =
        sqlx::query("UPDATE offers SET promoted = false WHERE id = $1")
            .bind(offer_id)
            .execute(&repo.db)
            .await;
}

/// 种的发布者（自审拦截用；行不在或无主 → None）。
pub async fn torrent_owner(
    repo: &Repo,
    torrent_id: i64,
) -> Result<Option<i64>, sqlx::Error> {
    sqlx::query_scalar("SELECT owner_id FROM torrents WHERE id = $1")
        .bind(torrent_id)
        .fetch_optional(&repo.db)
        .await
}

/// P-317 在架守卫：只在 `approval_status = 1` 时补 approved_at。
/// 返回 false = 种已下架/未过审，调用方须回滚 promoted。
pub async fn stamp_approved(
    repo: &Repo,
    torrent_id: i64,
) -> Result<bool, sqlx::Error> {
    let r: Option<i64> = sqlx::query_scalar(
        "UPDATE torrents SET approved_at = now() WHERE id = $1 \
         AND approval_status = 1 RETURNING id",
    )
    .bind(torrent_id)
    .fetch_optional(&repo.db)
    .await?;
    Ok(r.is_some())
}

/// 官种标签 id（当前字典 kind='official' scope='torrent'，取首个）。
pub async fn official_tag_ids(repo: &Repo) -> Result<Vec<i32>, sqlx::Error> {
    sqlx::query_scalar(
        "SELECT id FROM tag_dict \
         WHERE kind = 'official' AND scope = 'torrent' \
         ORDER BY id LIMIT 1",
    )
    .fetch_all(&repo.db)
    .await
}

/// 转正操作留痕（best-effort，失败不阻断）。
pub async fn log_promote(
    repo: &Repo,
    torrent_id: i64,
    operator_id: i64,
    offer_id: i64,
) {
    let _ = sqlx::query(
        "INSERT INTO torrent_operation_logs \
         (torrent_id, operator_id, action, detail) \
         VALUES ($1, $2, 'offer_promote', $3)",
    )
    .bind(torrent_id)
    .bind(operator_id)
    .bind(serde_json::json!({ "offer_id": offer_id }).to_string())
    .execute(&repo.db)
    .await;
}
