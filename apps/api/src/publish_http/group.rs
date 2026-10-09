//! 聚合组（0069）：同资源多版本归组 + 组订阅（0075）。
//! 从 publish_http.rs 按域拆出。

use actix_web::{get, post, web, HttpRequest, HttpResponse};
use serde::Deserialize;

use crate::dto::ok;
use crate::errors::{DomainError, DomainResult};
use crate::http::require_auth;
use crate::state::AppState;

#[post("/torrents/{id}/group")]
pub async fn group_attach(
    req: HttpRequest,
    state: web::Data<std::sync::Arc<AppState>>,
    path: web::Path<i64>,
    body: web::Json<GroupAttachReq>,
) -> DomainResult<HttpResponse> {
    let auth = require_auth(&req, &state).await?;
    let torrent_id = path.into_inner();
    let name = body.name.trim();
    if name.is_empty() || name.len() > 100 {
        return Err(DomainError::Validation("组名需 1-100 字".into()));
    }
    let owner: Option<i64> =
        sqlx::query_scalar("SELECT owner_id FROM torrents WHERE id = $1")
            .bind(torrent_id)
            .fetch_optional(&state.repo.db)
            .await
            .map_err(|e| DomainError::Internal(e.into()))?;
    let Some(owner) = owner else {
        return Err(DomainError::NotFound(torrent_id));
    };
    if owner != auth.id && auth.class_id < 90 {
        return Err(DomainError::Forbidden);
    }
    let gid: i64 = match sqlx::query_scalar::<_, i64>(
        "SELECT id FROM torrent_groups WHERE name = $1",
    )
    .bind(name)
    .fetch_optional(&state.repo.db)
    .await
    .map_err(|e| DomainError::Internal(e.into()))?
    {
        Some(g) => g,
        None => sqlx::query_scalar(
            "INSERT INTO torrent_groups (name, descr, created_by) \
             VALUES ($1, $2, $3) RETURNING id",
        )
        .bind(name)
        .bind(
            body.descr
                .as_deref()
                .map(str::trim)
                .filter(|d| !d.is_empty()),
        )
        .bind(auth.id)
        .fetch_one(&state.repo.db)
        .await
        .map_err(|e| DomainError::Internal(e.into()))?,
    };
    sqlx::query("UPDATE torrents SET group_id = $1 WHERE id = $2")
        .bind(gid)
        .bind(torrent_id)
        .execute(&state.repo.db)
        .await
        .map_err(|e| DomainError::Internal(e.into()))?;
    Ok(ok(serde_json::json!({ "group_id": gid, "name": name })))
}

/// 订阅聚合组（0075：新版本入组并过审时推送）
#[post("/torrents/groups/{group_id}/subscribe")]
pub async fn group_subscribe(
    req: HttpRequest,
    state: web::Data<std::sync::Arc<AppState>>,
    path: web::Path<i64>,
) -> DomainResult<HttpResponse> {
    let auth = require_auth(&req, &state).await?;
    let gid = path.into_inner();
    let exists: bool = sqlx::query_scalar(
        "SELECT EXISTS(SELECT 1 FROM torrent_groups WHERE id = $1)",
    )
    .bind(gid)
    .fetch_one(&state.repo.db)
    .await
    .unwrap_or(false);
    if !exists {
        return Err(DomainError::NotFound(gid));
    }
    sqlx::query(
        "INSERT INTO group_subscriptions (user_id, group_id) VALUES \
         ($1, $2) ON CONFLICT DO NOTHING",
    )
    .bind(auth.id)
    .bind(gid)
    .execute(&state.repo.db)
    .await
    .map_err(|e| DomainError::Internal(e.into()))?;
    Ok(ok(serde_json::json!({ "subscribed": gid })))
}

/// 退订聚合组
#[post("/torrents/groups/{group_id}/unsubscribe")]
pub async fn group_unsubscribe(
    req: HttpRequest,
    state: web::Data<std::sync::Arc<AppState>>,
    path: web::Path<i64>,
) -> DomainResult<HttpResponse> {
    let auth = require_auth(&req, &state).await?;
    let gid = path.into_inner();
    sqlx::query(
        "DELETE FROM group_subscriptions WHERE user_id = $1 AND group_id = $2",
    )
    .bind(auth.id)
    .bind(gid)
    .execute(&state.repo.db)
    .await
    .map_err(|e| DomainError::Internal(e.into()))?;
    Ok(ok(serde_json::json!({ "unsubscribed": gid })))
}

/// 组详情 + 组内全部过审版本（详情页「同组资源」数据源；未入组返回 group=null）
#[get("/torrents/{id}/group")]
pub async fn group_info(
    state: web::Data<std::sync::Arc<AppState>>,
    path: web::Path<i64>,
) -> DomainResult<HttpResponse> {
    let torrent_id = path.into_inner();
    // torrents.group_id 可空：fetch_optional 得 Option<Option<i64>>——外层 None=行不存在，
    // 内层 None=未挂组。旧版直接解一层 Option，group_id 为 NULL 时把内层 None 当
    // 行不存在之外还触发 sqlx「unexpected null」解码错（500）。显式双层解构。
    let group_id: Option<i64> =
        sqlx::query_scalar("SELECT group_id FROM torrents WHERE id = $1")
            .bind(torrent_id)
            .fetch_optional(&state.repo.db)
            .await
            .map_err(|e| DomainError::Internal(e.into()))?
            .flatten();
    let Some(gid) = group_id else {
        return Ok(ok(serde_json::json!({ "group": null })));
    };
    let row: Option<(String, Option<String>, Option<i32>)> = sqlx::query_as(
        "SELECT name, descr, category_id FROM torrent_groups WHERE id = $1",
    )
    .bind(gid)
    .fetch_optional(&state.repo.db)
    .await
    .map_err(|e| DomainError::Internal(e.into()))?;
    let Some((name, descr, category_id)) = row else {
        return Ok(ok(serde_json::json!({ "group": null })));
    };
    let items: Vec<(i64, String, Option<String>, i64, i32, i32, i32, bool, Option<String>)> =
        sqlx::query_as(
            "SELECT t.id, t.name, t.small_descr, t.size, t.seeders, \
                    t.leechers, t.times_completed, t.official_tag, s.std \
             FROM torrents t \
             LEFT JOIN LATERAL ( \
                 SELECT d.name AS std FROM torrent_sections ts \
                 JOIN section_dict d ON d.kind = ts.kind \
                   AND d.id = ts.dict_id \
                 WHERE ts.torrent_id = t.id AND ts.kind = 'standard' \
                 LIMIT 1 \
             ) s ON TRUE \
             WHERE t.group_id = $1 AND t.approval_status = 1 \
             ORDER BY t.id",
        )
        .bind(gid)
        .fetch_all(&state.repo.db)
        .await
        .map_err(|e| DomainError::Internal(e.into()))?;
    let items: Vec<_> = items
        .into_iter()
        .map(|(id, n, sd, size, s, l, c, official, std)| {
            serde_json::json!({
                "id": id, "name": n, "small_descr": sd, "size": size,
                "seeders": s, "leechers": l, "times_completed": c,
                "official": official, "current": id == torrent_id,
                "tier": tier_of(std.as_deref()),
            })
        })
        .collect();
    Ok(ok(serde_json::json!({
        "group": { "id": gid, "name": name, "descr": descr, "category_id": category_id },
        "items": items,
    })))
}

// ============ 聚合组（0069：同一资源多版本，GZ Torrent Group 的教育域映射） ============

#[derive(Deserialize)]
struct GroupAttachReq {
    name: String,
    #[serde(default)]
    descr: Option<String>,
}

/// 多版本分槽（0336 G2）：由 `standard` 维度值**派生** tier，零 schema 改动
/// （PTP 的分槽语义；不存列、不 match site_type，读路径算）。
/// 2160p/4K→UHD、1080p→HD、720p/480p→SD、其余或缺失→Other。
fn tier_of(std_name: Option<&str>) -> &'static str {
    let Some(s) = std_name else { return "Other" };
    let s = s.to_ascii_lowercase();
    if s.contains("2160") || s.contains("4k") || s.contains("uhd") {
        "UHD"
    } else if s.contains("1080") {
        "HD"
    } else if s.contains("720") || s.contains("480") {
        "SD"
    } else {
        "Other"
    }
}
