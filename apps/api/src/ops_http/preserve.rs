//! M19 保种区 + 插件总览 + 死种复活（列表/认领/我的）。
//! 从 ops_http.rs 按域拆出。

use actix_web::{get, post, web, HttpRequest, HttpResponse, Responder};
use serde::Deserialize;

use crate::dto::ok;
use crate::errors::{DomainError, DomainResult};
use crate::http::require_auth;
use crate::state::AppState;

/// 保种区列表（包子站 requireseed.php 口径）：统计六格 + 筛选 + 分页 + 种子九列表格
#[get("/preserve")]
pub(super) async fn preserve_list(
    state: web::Data<std::sync::Arc<AppState>>,
    q: web::Query<PreserveQuery>,
) -> DomainResult<impl Responder> {
    let page = q.page.unwrap_or(0).max(0);
    let per = 50i64;
    let kw = q
        .keyword
        .as_deref()
        .map(|k| crate::http::like_pattern(&k))
        .unwrap_or_else(|| "%".into());
    // 审计修复（P1）：scope/status 筛选此前声明即弃（#[allow(dead_code)]），前端表单
    // 提交被静默忽略。接线：scope=official/general 过滤官种位；status 按认领/延续
    // 状态过滤（current=全部在保、active=已被认领延续、grace=移出宽限中、expired=已移出）。
    let scope_ok =
        matches!(q.scope.as_deref(), Some("official") | Some("general"));
    let official_only = q.scope.as_deref() == Some("official");
    let general_only = q.scope.as_deref() == Some("general");
    // status=expired 需要查已移出行——主查询固定 exited_at IS NULL，expired 单独走分支
    if q.status.as_deref() == Some("expired") {
        let rows: Vec<(i64, String, i64)> = sqlx::query_as(
            "SELECT sp.torrent_id, t.name, t.size FROM seed_preserve sp \
             JOIN torrents t ON t.id = sp.torrent_id \
             WHERE sp.exited_at IS NOT NULL \
               AND (NOT $1::bool OR t.official_tag) \
               AND (NOT $2::bool OR NOT t.official_tag) \
             ORDER BY sp.exited_at DESC LIMIT 50",
        )
        .bind(official_only)
        .bind(general_only)
        .fetch_all(&state.repo.db)
        .await
        .map_err(|e| DomainError::Internal(e.into()))?;
        return Ok(ok(rows));
    }
    let _ = scope_ok;

    let rows = sqlx::query_as::<_, PreserveRow>(
        "SELECT sp.torrent_id, t.name, t.size, t.seeders, u.username AS claimed_by, \
         t.small_descr, t.category_id, t.medium_id, t.grade_id, t.edition_id, \
         t.leechers, t.times_completed, \
         (SELECT count(*) FROM comments c WHERE c.torrent_id = t.id) AS comments, \
         t.official_tag, t.sticky, t.anonymous, \
         (SELECT p.kind::text FROM promotions p WHERE p.starts_at <= now() AND p.ends_at > now() \
            AND (
                    p.torrent_id = t.id
                    OR (p.torrent_id IS NULL AND (
                        p.scope = 'global'
                        OR (p.scope = 'official' AND t.official_tag)
                        OR (p.scope = 'non_official' AND NOT t.official_tag)
                        OR (p.scope = 'category' AND t.category_id = p.category_id)))) \
            ORDER BY CASE p.kind::text WHEN 'x2free' THEN 6 WHEN 'x2half' THEN 5 WHEN 'x2' THEN 4 WHEN 'free' THEN 3 WHEN 'half' THEN 2 WHEN 'p30' THEN 1 ELSE 0 END DESC, p.id DESC LIMIT 1) AS promotion, \
         (SELECT p.ends_at FROM promotions p WHERE p.starts_at <= now() AND p.ends_at > now() \
            AND (
                    p.torrent_id = t.id
                    OR (p.torrent_id IS NULL AND (
                        p.scope = 'global'
                        OR (p.scope = 'official' AND t.official_tag)
                        OR (p.scope = 'non_official' AND NOT t.official_tag)
                        OR (p.scope = 'category' AND t.category_id = p.category_id)))) \
            ORDER BY CASE p.kind::text WHEN 'x2free' THEN 6 WHEN 'x2half' THEN 5 WHEN 'x2' THEN 4 WHEN 'free' THEN 3 WHEN 'half' THEN 2 WHEN 'p30' THEN 1 ELSE 0 END DESC, p.id DESC LIMIT 1) AS promotion_ends_at, \
         t.media_info->>'poster' AS poster, \
         CASE WHEN t.anonymous THEN NULL ELSE o.username END AS owner_name, \
         t.created_at \
         FROM seed_preserve sp \
         JOIN torrents t ON t.id = sp.torrent_id \
         LEFT JOIN users u ON u.id = sp.claimed_by \
         LEFT JOIN users o ON o.id = t.owner_id \
         WHERE sp.exited_at IS NULL AND t.approval_status = 1 \
           AND (NOT $5::bool OR t.official_tag) \
           AND (NOT $6::bool OR NOT t.official_tag) \
           AND (NOT $7::bool OR sp.claimed_by IS NOT NULL) \
           AND ($1::int IS NULL OR t.category_id = $1) \
           AND t.name ILIKE $2 \
         ORDER BY t.seeders ASC, sp.torrent_id LIMIT $3 OFFSET $4",
    )
    .bind(q.category)
    .bind(kw)
    .bind(per)
    .bind(page * per)
    .bind(official_only)
    .bind(general_only)
    .bind(q.status.as_deref() == Some("active"))
    .fetch_all(&state.repo.db)
    .await
    .map_err(|e| DomainError::Internal(e.into()))?;

    // 统计六格（包子站保种区口径：保种中/延续中/官方保种/普通保种/今日新增/今日移出）
    let (preserving, _exited, total, today_in, today_out): (i64, i64, i64, i64, i64) =
        sqlx::query_as(
            "SELECT \
                (SELECT count(*) FROM seed_preserve WHERE exited_at IS NULL), \
                (SELECT count(*) FROM seed_preserve WHERE exited_at IS NOT NULL), \
                (SELECT count(*) FROM seed_preserve WHERE exited_at IS NULL), \
                (SELECT count(*) FROM seed_preserve WHERE claimed_at > now() - interval '1 day'), \
                (SELECT count(*) FROM seed_preserve WHERE exited_at > now() - interval '1 day')",
        )
        .fetch_one(&state.repo.db)
        .await
        .map_err(|e| DomainError::Internal(e.into()))?;
    let official = sqlx::query_scalar::<_, i64>(
        "SELECT count(*) FROM seed_preserve sp JOIN torrents t ON t.id = sp.torrent_id \
         WHERE sp.exited_at IS NULL AND t.official_tag",
    )
    .fetch_one(&state.repo.db)
    .await
    .unwrap_or(0);
    // 延续中 = 已被认领且仍在保种区（此前硬编码 0 永远显示 0）
    let continued: i64 = sqlx::query_scalar::<_, i64>(
        "SELECT count(*) FROM seed_preserve WHERE exited_at IS NULL \
         AND claimed_by IS NOT NULL",
    )
    .fetch_one(&state.repo.db)
    .await
    .unwrap_or(0);

    Ok(ok(serde_json::json!({
        "items": rows,
        "total": total,
        "stats": {
            "preserving": preserving, "continued": continued,
            "official": official, "general": (preserving - official).max(0),
            "today_in": today_in, "today_out": today_out,
        },
        "page": page, "per_page": per,
    })))
}

#[derive(Deserialize)]
struct PreserveClaimReq {
    torrent_id: i64,
}

#[post("/preserve/claim")]
pub(super) async fn preserve_claim(
    req: HttpRequest,
    state: web::Data<std::sync::Arc<AppState>>,
    body: web::Json<PreserveClaimReq>,
) -> DomainResult<HttpResponse> {
    let auth = require_auth(&req, &state).await?;
    // 认领即记录做种时长/上传量基线（NP claims.seed_time_begin/uploaded_begin 口径）
    let updated = sqlx::query(
        "UPDATE seed_preserve sp SET \
            claimed_by = $2, claimed_at = now(), last_settle_at = now(), \
            seed_time_begin = COALESCE(( \
                SELECT s.seeded_seconds FROM snatches s \
                WHERE s.torrent_id = sp.torrent_id AND s.user_id = $2), 0), \
            uploaded_begin = COALESCE(( \
                SELECT s.uploaded FROM snatches s \
                WHERE s.torrent_id = sp.torrent_id AND s.user_id = $2), 0) \
         WHERE sp.torrent_id = $1 AND sp.claimed_by IS NULL",
    )
    .bind(body.torrent_id)
    .bind(auth.id)
    .execute(&state.repo.db)
    .await
    .map_err(|e| DomainError::Internal(e.into()))?;
    if updated.rows_affected() == 0 {
        return Err(DomainError::Validation(
            "该种子已被认领或不在保种区".into(),
        ));
    }
    state
        .repo
        .audit(Some(auth.id), "preserve_claim", Some(body.torrent_id))
        .await;
    Ok(ok(serde_json::json!({ "claimed": body.torrent_id })))
}

// ============ M28 插件 ============

/// 插件清单（staff 可见）
#[get("/admin/plugins")]
pub(super) async fn plugins_overview(
    req: HttpRequest,
    state: web::Data<std::sync::Arc<AppState>>,
) -> DomainResult<impl Responder> {
    let auth = require_auth(&req, &state).await?;
    crate::authz::require_perm(
        &state,
        &auth,
        crate::authz::perm::PLUGINS_MANAGE,
    )
    .await?;
    Ok(ok(serde_json::json!({
        "plugins": state.plugins.list(),
        "hooks": ["on_user_login", "on_torrent_upload", "on_seeding_milestone"],
    })))
}

// ============ 复活任务（0073，U3D Graveyard 口径） ============

#[derive(sqlx::FromRow, serde::Serialize)]
struct PreserveRow {
    torrent_id: i64,
    name: String,
    size: i64,
    seeders: i32,
    claimed_by: Option<String>,
    // 资源库行同构字段（保种区列表复用 TorrentTr 渲染，参考站口径）
    small_descr: Option<String>,
    category_id: i32,
    medium_id: Option<i32>,
    grade_id: Option<i32>,
    edition_id: Option<i32>,
    leechers: i32,
    times_completed: i32,
    comments: i64,
    #[serde(rename = "official")]
    official_tag: bool,
    sticky: bool,
    anonymous: bool,
    promotion: Option<String>,
    promotion_ends_at: Option<chrono::DateTime<chrono::Utc>>,
    poster: Option<String>,
    owner_name: Option<String>,
    created_at: chrono::DateTime<chrono::Utc>,
}

#[derive(Deserialize)]
struct PreserveQuery {
    /// scope: all/official/general；status: current/active/grace/expired
    #[serde(default)]
    scope: Option<String>,
    #[serde(default)]
    status: Option<String>,
    #[serde(default)]
    category: Option<i32>,
    #[serde(default)]
    keyword: Option<String>,
    #[serde(default)]
    page: Option<i64>,
}
