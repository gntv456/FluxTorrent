use actix_web::{get, post, put, web, HttpRequest, HttpResponse};
use serde::Deserialize;

use crate::dto::ok;
use crate::errors::{DomainError, DomainResult};
use crate::state::AppState;

use super::guard::staff;

// ============ 管理组面板（staffpanel.php 复刻） + 站点设定 ============

#[derive(serde::Serialize, sqlx::FromRow)]
struct StaffPanelEntry {
    section: String,
    name: String,
    url: String,
    info: String,
    tab_key: String,
    min_class: i32,
    /// 权限键（0209）：非空时还须 user_can(perm_key)；NULL = 仅按 min_class
    #[serde(skip_serializing_if = "Option::is_none")]
    #[sqlx(default)]
    perm_key: Option<String>,
}

/// 管理组面板：职能分组（section）+ 细粒度等级过滤（min_class）
/// 说明：旧的 panel 字段（sysop/admin/moderator）是权限等级，曾被当作分组维度，
/// 导致同一职能被拆散；现改为按 section 分组、按 min_class 逐条过滤，无权条目直接不返回。
/// 0209：perm_key 非空的条目再按 user_can 过滤——「给某人只开某个面板」
/// 走用户级权限覆盖（显式授予/拒绝），导航不再整块按等级切。
#[get("/admin/staffpanel")]
async fn staff_panel(
    req: HttpRequest,
    state: web::Data<std::sync::Arc<AppState>>,
) -> DomainResult<HttpResponse> {
    let auth = staff(&req, &state).await?;
    let rows: Vec<StaffPanelEntry> =
        sqlx::query_as::<_, StaffPanelEntry>(&format!(
            "SELECT section, name, url, info, tab_key, min_class, perm_key \
         FROM staff_panel_entries \
         WHERE min_class <= $1 AND {} \
         ORDER BY section, sort, id",
            crate::modules::module_on_sql("staff_panel_entries")
        ))
        .bind(auth.class_id)
        .fetch_all(&state.repo.db)
        .await
        .map_err(|e| DomainError::Internal(e.into()))?;
    // 权限键过滤（0209）：逐条 user_can；无键条目直接放行
    let mut filtered: Vec<StaffPanelEntry> = Vec::new();
    for e in rows {
        match e.perm_key.as_deref() {
            Some(pk) => {
                if crate::authz::can(&state, &auth, pk).await {
                    filtered.push(e);
                }
            }
            None => filtered.push(e),
        }
    }
    Ok(ok(serde_json::json!({
        "entries": filtered,
        "role": if auth.class_id >= 99 { "sysop" }
            else if auth.class_id >= 93 { "administrator" }
            else { "moderator" },
        "class_id": auth.class_id,
    })))
}

// ============ 面板条目 CRUD（0209 P1-7：加减导航项不再写 SQL） ============

#[derive(sqlx::FromRow, serde::Serialize)]
struct StaffPanelEntryFull {
    id: i32,
    section: String,
    name: String,
    url: String,
    info: String,
    sort: i32,
    tab_key: String,
    min_class: i32,
    #[serde(skip_serializing_if = "Option::is_none")]
    #[sqlx(default)]
    module_key: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    #[sqlx(default)]
    perm_key: Option<String>,
}

#[get("/admin/staffpanel-entries")]
async fn staff_panel_entries_list(
    req: HttpRequest,
    state: web::Data<std::sync::Arc<AppState>>,
) -> DomainResult<HttpResponse> {
    let auth = staff(&req, &state).await?;
    crate::authz::require_perm(
        &state,
        &auth,
        crate::authz::perm::SETTINGS_MANAGE,
    )
    .await?;
    let rows: Vec<StaffPanelEntryFull> = sqlx::query_as(
        "SELECT id, section, name, url, info, sort, tab_key, min_class, \
         module_key, perm_key FROM staff_panel_entries ORDER BY section, sort, id",
    )
    .fetch_all(&state.repo.db)
    .await
    .map_err(|e| DomainError::Internal(e.into()))?;
    Ok(ok(rows))
}

#[derive(Deserialize)]
struct PanelEntryReq {
    section: String,
    name: String,
    url: String,
    info: String,
    #[serde(default)]
    sort: Option<i32>,
    #[serde(default)]
    tab_key: Option<String>,
    #[serde(default)]
    min_class: Option<i32>,
    #[serde(default)]
    module_key: Option<String>,
    #[serde(default)]
    perm_key: Option<String>,
}

#[post("/admin/staffpanel-entries")]
async fn staff_panel_entries_add(
    req: HttpRequest,
    state: web::Data<std::sync::Arc<AppState>>,
    body: web::Json<PanelEntryReq>,
) -> DomainResult<HttpResponse> {
    let auth = staff(&req, &state).await?;
    crate::authz::require_perm(
        &state,
        &auth,
        crate::authz::perm::SETTINGS_MANAGE,
    )
    .await?;
    if body.name.trim().is_empty() || body.url.trim().is_empty() {
        return Err(DomainError::Validation("面板条目需要名称与 URL".into()));
    }
    let id: i32 = sqlx::query_scalar(
        "INSERT INTO staff_panel_entries \
         (section, name, url, info, sort, tab_key, min_class, module_key, perm_key) \
         VALUES ($1, $2, $3, $4, COALESCE($5, 0), COALESCE($6, ''), COALESCE($7, 90), $8, $9) \
         RETURNING id",
    )
    .bind(body.section.trim())
    .bind(body.name.trim())
    .bind(body.url.trim())
    .bind(body.info.trim())
    .bind(body.sort)
    .bind(body.tab_key.as_deref().map(str::trim))
    .bind(body.min_class)
    .bind(body.module_key.as_deref().map(str::trim).filter(|s| !s.is_empty()))
    .bind(body.perm_key.as_deref().map(str::trim).filter(|s| !s.is_empty()))
    .fetch_one(&state.repo.db)
    .await
    .map_err(|e| DomainError::Internal(e.into()))?;
    state
        .repo
        .audit(Some(auth.id), "staffpanel.entry_add", Some(id as i64))
        .await;
    Ok(ok(serde_json::json!({ "id": id })))
}

#[put("/admin/staffpanel-entries/{id}")]
async fn staff_panel_entries_update(
    req: HttpRequest,
    state: web::Data<std::sync::Arc<AppState>>,
    path: web::Path<i32>,
    body: web::Json<PanelEntryReq>,
) -> DomainResult<HttpResponse> {
    let auth = staff(&req, &state).await?;
    crate::authz::require_perm(
        &state,
        &auth,
        crate::authz::perm::SETTINGS_MANAGE,
    )
    .await?;
    let id = path.into_inner();
    let n = sqlx::query(
        "UPDATE staff_panel_entries SET section=$2, name=$3, url=$4, info=$5, \
         sort=COALESCE($6, sort), tab_key=COALESCE($7, tab_key), \
         min_class=COALESCE($8, min_class), module_key=$9, perm_key=$10 \
         WHERE id=$1",
    )
    .bind(id)
    .bind(body.section.trim())
    .bind(body.name.trim())
    .bind(body.url.trim())
    .bind(body.info.trim())
    .bind(body.sort)
    .bind(body.tab_key.as_deref().map(str::trim))
    .bind(body.min_class)
    .bind(
        body.module_key
            .as_deref()
            .map(str::trim)
            .filter(|s| !s.is_empty()),
    )
    .bind(
        body.perm_key
            .as_deref()
            .map(str::trim)
            .filter(|s| !s.is_empty()),
    )
    .execute(&state.repo.db)
    .await
    .map_err(|e| DomainError::Internal(e.into()))?
    .rows_affected();
    if n == 0 {
        return Err(DomainError::NotFound(id as i64));
    }
    state
        .repo
        .audit(Some(auth.id), "staffpanel.entry_update", Some(id as i64))
        .await;
    Ok(ok(serde_json::json!({ "ok": true })))
}

#[derive(Deserialize)]
struct PanelEntryDelReq {
    id: i32,
}

#[post("/admin/staffpanel-entries/delete")]
async fn staff_panel_entries_delete(
    req: HttpRequest,
    state: web::Data<std::sync::Arc<AppState>>,
    body: web::Json<PanelEntryDelReq>,
) -> DomainResult<HttpResponse> {
    let auth = staff(&req, &state).await?;
    crate::authz::require_perm(
        &state,
        &auth,
        crate::authz::perm::SETTINGS_MANAGE,
    )
    .await?;
    let n = sqlx::query("DELETE FROM staff_panel_entries WHERE id = $1")
        .bind(body.id)
        .execute(&state.repo.db)
        .await
        .map_err(|e| DomainError::Internal(e.into()))?
        .rows_affected();
    if n == 0 {
        return Err(DomainError::NotFound(body.id as i64));
    }
    state
        .repo
        .audit(Some(auth.id), "staffpanel.entry_del", Some(body.id as i64))
        .await;
    Ok(ok(serde_json::json!({ "deleted": body.id })))
}

#[derive(serde::Serialize, sqlx::FromRow)]
struct SiteSettingRow {
    name: String,
    value: String,
    updated_at: chrono::DateTime<chrono::Utc>,
    descr: Option<String>,
    grp: Option<String>,
}

/// 站点设定：sysop（99）只读 + 可写；administrator 只读
#[get("/admin/settings")]
async fn site_settings_get(
    req: HttpRequest,
    state: web::Data<std::sync::Arc<AppState>>,
) -> DomainResult<HttpResponse> {
    let auth = staff(&req, &state).await?;
    // 密文脱敏：旧版全量回值把 SMTP 密码等密文明文泄露给任何 90+。
    // 口径与新协议 settings/schema 一致——secret 字段只回「已设置」状态。
    let rows: Vec<SiteSettingRow> = sqlx::query_as(
                "SELECT s.name, \
         CASE WHEN COALESCE(m.secret, false) THEN '' ELSE s.value END AS value, \
         s.updated_at, s.descr, \
         COALESCE(s.grp, 'misc') AS grp FROM site_settings s LEFT JOIN settings_meta m ON m.name = s.name ORDER BY s.grp, \
         s.name",
    )
    .fetch_all(&state.repo.db)
    .await
    .map_err(|e| DomainError::Internal(e.into()))?;
    Ok(ok(serde_json::json!({
        "settings": rows,
        "editable": auth.class_id >= 99,
    })))
}

#[derive(serde::Serialize, sqlx::FromRow)]
struct CheaterRow {
    user_id: i64,
    username: String,
    torrent_id: Option<i64>,
    name: Option<String>,
    upspeed: i64,
    uploaded_delta: i64,
    announced_at: chrono::DateTime<chrono::Utc>,
}

/// 作弊者探测（cheaters.php 口径）：snatches 实时上报速度 / 短窗上传增量超阈值的会话
#[get("/admin/cheaters")]
async fn cheaters_scan(
    req: HttpRequest,
    state: web::Data<std::sync::Arc<AppState>>,
) -> DomainResult<HttpResponse> {
    let _auth = staff(&req, &state).await?;
    // 阈值：上报速度 > 100 MB/s 或 1 小时内上传增量 > 500 GB 判为可疑
    // 口径：近 1 小时上传增量折算平均速度 > 100 MB/s，或 1 小时增量绝对值 > 500 GB
    let rows: Vec<CheaterRow> = sqlx::query_as(
                "SELECT s.user_id, u.username, s.torrent_id, t.name, \
         (g.uploaded_delta / 3600)::bigint AS upspeed, \
         COALESCE(g.uploaded_delta, 0)::bigint AS uploaded_delta, \
         now() AS announced_at FROM snatches s JOIN users u ON u.id = s.user_id LEFT JOIN torrents t ON t.id = s.torrent_id JOIN LATERAL ( SELECT COALESCE(sum(tl.delta_up), 0)::bigint AS uploaded_delta FROM traffic_ledger tl WHERE tl.user_id = s.user_id AND tl.torrent_id = s.torrent_id AND tl.window_start > now() - interval '1 hour' ) g ON TRUE WHERE (s.seeding OR s.leeching) AND (g.uploaded_delta > 536870912000 OR g.uploaded_delta / 3600 > 104857600) ORDER BY g.uploaded_delta DESC LIMIT 100",
    )
    .fetch_all(&state.repo.db)
    .await
    .map_err(|e| DomainError::Internal(e.into()))?;
    Ok(ok(rows))
}

#[derive(Deserialize)]
struct SettingPut {
    name: String,
    value: String,
}

/// 修改单项站点设定（仅 sysop）
#[put("/admin/settings")]
async fn site_settings_put(
    req: HttpRequest,
    state: web::Data<std::sync::Arc<AppState>>,
    body: web::Json<SettingPut>,
) -> DomainResult<HttpResponse> {
    let auth = staff(&req, &state).await?;
    crate::authz::require_perm(
        &state,
        &auth,
        crate::authz::perm::SETTINGS_MANAGE,
    )
    .await?;
    if body.name.trim().is_empty() || body.value.len() > 4096 {
        return Err(DomainError::Validation("非法的设定项".into()));
    }
    // 三审 B-4：site_type 是站型包 apply 的派生锚点——设置页直改只改字符串，
    // 分类/模块/维度全不跟随，设置值与站点形态脱钩。拒绝并引导走站型切换。
    if body.name == "site_type" {
        return Err(DomainError::Validation(
            "站点类型请在后台「分类管理 → 站点类型包」切换（会完整应用分类/模块/维度）".into(),
        ));
    }
    let updated = sqlx::query(
        "UPDATE site_settings SET value = $3, updated_at = now() \
         WHERE name = $2 RETURNING name",
    )
    .bind(auth.id)
    .bind(body.name.trim())
    .bind(body.value.trim())
    .fetch_optional(&state.repo.db)
    .await
    .map_err(|e| DomainError::Internal(e.into()))?;
    if updated.is_none() {
        return Err(DomainError::Validation("设定项不存在".into()));
    }
    state
        .repo
        .audit(Some(auth.id), "site_setting_update", None)
        .await;
    Ok(ok(serde_json::json!({ "ok": true })))
}
