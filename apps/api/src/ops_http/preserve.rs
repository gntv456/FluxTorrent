//! M19 保种区 + 插件总览 + 死种复活（列表/认领/我的）。
//! 从 ops_http.rs 按域拆出。

use actix_web::{get, post, web, HttpRequest, HttpResponse, Responder};
use serde::Deserialize;

use crate::dto::ok;
use crate::errors::{DomainError, DomainResult};
use crate::http::require_auth;
use crate::state::AppState;

/// 保种区「移出宽限」证据：worker 出种时给该种挂 3 天免费促销（source=preserve_grace）。
/// `{j}` 处填 LATERAL 别名引用前缀。列表与计数共用，两处各写一份必然漂移。
const GRACE_JOIN: &str = "LEFT JOIN LATERAL \
     (SELECT p.ends_at FROM promotions p \
      WHERE p.source = 'preserve_grace' AND p.torrent_id = sp.torrent_id \
      ORDER BY p.ends_at DESC LIMIT 1) gr ON true";

/// 四档视图判定片段，`{m}` 由调用方填占位符编号（列表/计数的参数序不同）。
/// in=在保全部（默认）、active=保种中（已认领）、grace=延续中（已移出但宽限未过）、
/// out=已移出（宽限已过或从未挂宽限）。
const VIEW_FILTER: &str = "(   ({m} = 'in' AND sp.exited_at IS NULL) \
     OR ({m} = 'active' AND sp.exited_at IS NULL \
         AND sp.claimed_by IS NOT NULL) \
     OR ({m} = 'grace' AND sp.exited_at IS NOT NULL \
         AND gr.ends_at > now()) \
     OR ({m} = 'out' AND sp.exited_at IS NOT NULL \
         AND (gr.ends_at IS NULL OR gr.ends_at <= now())))";

/// 保种区列表（同类站 requireseed.php 口径）：统计六格 + 筛选 + 分页 + 种子九列表格
#[get("/preserve")]
pub(super) async fn preserve_list(
    state: web::Data<std::sync::Arc<AppState>>,
    q: web::Query<PreserveQuery>,
) -> DomainResult<impl Responder> {
    // 分页与全站列表同口径（dto::page_window 是 1 基）。此前这里按 0 基算 offset，
    // 客户端照惯例传 page=1 时翻到的是第二页——保种区首页整页空白，
    // 而 total 明明 >0。
    let page = q.page.unwrap_or(1).max(1);
    let per = 50i64;
    let offset = (page - 1) * per;
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
    let _ = scope_ok;
    // 视图档位（此前 grace 选中即被忽略、expired 走独立分支只回三列裸数组，
    // 前端按 {items,total,stats} 解构 → 选「已移出」整页 500）
    let mode = match q.status.as_deref() {
        Some("active") => "active",
        Some("grace") => "grace",
        Some("expired") => "out",
        _ => "in",
    };

    // 促销角标：与前台列表/详情、worker 计费共用「逐维合并」片段（P-120 配套，
    // 旧写法在此内联两份按强度取最强档的子查询，多条促销并存时会与计费漂移）
    let ekind = crate::torrents::promo::effective_kind();
    let eends = crate::torrents::promo::effective_ends();
    let rows = sqlx::query_as::<_, PreserveRow>(
        &format!(
            "SELECT sp.torrent_id, t.name, t.size, t.seeders, u.username AS claimed_by, \
         t.small_descr, t.category_id, t.medium_id, t.grade_id, t.edition_id, \
         t.leechers, t.times_completed, \
         (SELECT count(*) FROM comments c WHERE c.torrent_id = t.id) AS comments, \
         t.official_tag, t.sticky, t.anonymous, \
         {ekind} AS promotion, \
         {eends} AS promotion_ends_at, \
         t.media_info->>'poster' AS poster, \
         CASE WHEN t.anonymous THEN NULL ELSE o.username END AS owner_name, \
         t.created_at \
             FROM seed_preserve sp \
             JOIN torrents t ON t.id = sp.torrent_id \
             LEFT JOIN users u ON u.id = sp.claimed_by \
             LEFT JOIN users o ON o.id = t.owner_id \
             {grace} \
             WHERE t.approval_status = 1 \
               AND (NOT $5::bool OR t.official_tag) \
               AND (NOT $6::bool OR NOT t.official_tag) \
               AND ($1::int IS NULL OR t.category_id = $1) \
               AND t.name ILIKE $2 \
               AND {view} \
             ORDER BY CASE WHEN sp.exited_at IS NULL \
                       THEN t.seeders ELSE 0 END ASC, \
                      COALESCE(sp.exited_at, t.created_at) DESC, sp.torrent_id \
             LIMIT $3 OFFSET $4",
            grace = GRACE_JOIN,
            view = VIEW_FILTER.replace("{m}", "$7"),
        ),
    )
    .bind(q.category)
    .bind(kw.as_str())
    .bind(per)
    .bind(offset)
    .bind(official_only)
    .bind(general_only)
    .bind(mode)
    .fetch_all(&state.repo.db)
    .await
    .map_err(|e| DomainError::Internal(e.into()))?;

    // 「共 N 条」与当前视图/筛选同源。此前 total 恒等于「全部在保」行数，
    // 与筛选结果无关——换一档筛选，计数就说谎。
    let total: i64 = sqlx::query_scalar(&format!(
        "SELECT count(*) FROM seed_preserve sp \
         JOIN torrents t ON t.id = sp.torrent_id \
         {grace} \
         WHERE t.approval_status = 1 \
           AND (NOT $1::bool OR t.official_tag) \
           AND (NOT $2::bool OR NOT t.official_tag) \
           AND ($3::int IS NULL OR t.category_id = $3) \
           AND t.name ILIKE $4 AND {view}",
        grace = GRACE_JOIN,
        view = VIEW_FILTER.replace("{m}", "$5"),
    ))
    .bind(official_only)
    .bind(general_only)
    .bind(q.category)
    .bind(kw.as_str())
    .bind(mode)
    .fetch_one(&state.repo.db)
    .await
    .map_err(|e| DomainError::Internal(e.into()))?;

    // 统计六格（同类站保种区口径：保种中/延续中/官方保种/普通保种/今日新增/今日移出）
    let (preserving, _exited, today_in, today_out): (i64, i64, i64, i64) =
        sqlx::query_as(
            "SELECT \
                (SELECT count(*) FROM seed_preserve WHERE exited_at IS NULL), \
                (SELECT count(*) FROM seed_preserve WHERE exited_at IS NOT NULL), \
                (SELECT count(*) FROM seed_preserve WHERE claimed_at \
                   > now() - interval '1 day'), \
                (SELECT count(*) FROM seed_preserve WHERE exited_at \
                   > now() - interval '1 day')",
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
