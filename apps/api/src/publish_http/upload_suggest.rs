//! 发种后的聚合组推荐（0288 从 upload.rs 拆出，300 行门禁）。
//!
//! 0075 口径，未显式指定组时给客户端三条线索：
//!   a) `pieces_hash` 命中已有组 → 建议直接锁定（跨站同源再发布场景）
//!   b) 否则 trgm 名称相似度 > 0.4 的组 → 候选列表
//!   c) 无组可荐但 `pieces_hash` 已有同源种（0284 P1-4）→ `same_source` 提示
//! 返回 `null` 表示没有任何线索可给。

use sqlx::PgPool;

/// 站长可用 `upload_dup_policy=group` 把 a) 从「建议」升级为「自动并组」，
/// 那条判定在 `upload_precheck::dup_policy`（入库前），与此处提示不冲突。
pub(super) async fn group_suggest(
    db: &PgPool,
    pieces_hash: &str,
    name: &str,
    id: i64,
    group_id: Option<i64>,
) -> serde_json::Value {
    if group_id.is_some() {
        return serde_json::json!(null);
    }
    let lock: Option<i64> = sqlx::query_scalar(
        "SELECT t2.group_id FROM torrents t2 WHERE t2.pieces_hash = $1 \
         AND t2.pieces_hash <> '' AND t2.group_id IS NOT NULL LIMIT 1",
    )
    .bind(pieces_hash)
    .fetch_optional(db)
    .await
    .unwrap_or(None);
    if let Some(gid) = lock {
        let gname: String =
            sqlx::query_scalar("SELECT name FROM torrent_groups WHERE id = $1")
                .bind(gid)
                .fetch_one(db)
                .await
                .unwrap_or_default();
        return serde_json::json!({
            "locked": true, "group_id": gid, "name": gname,
        });
    }
    let cands: Vec<(i64, String)> = sqlx::query_as(
        "SELECT g.id, g.name FROM torrent_groups g \
         WHERE similarity(g.name, $1) > 0.4 \
         ORDER BY similarity(g.name, $1) DESC LIMIT 3",
    )
    .bind(name)
    .fetch_all(db)
    .await
    .unwrap_or_default();
    if !cands.is_empty() {
        return serde_json::json!({ "locked": false, "candidates": cands });
    }
    let same: Option<(i64, String)> = sqlx::query_as(
        "SELECT id, name FROM torrents WHERE pieces_hash = $1 \
         AND pieces_hash <> '' AND id <> $2 ORDER BY id LIMIT 1",
    )
    .bind(pieces_hash)
    .bind(id)
    .fetch_optional(db)
    .await
    .unwrap_or(None);
    match same {
        Some((sid, sname)) => serde_json::json!({
            "same_source": true, "torrent_id": sid, "name": sname,
        }),
        None => serde_json::json!(null),
    }
}
