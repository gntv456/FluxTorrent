use actix_web::{get, put, web, HttpRequest, HttpResponse};
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
}

/// 管理组面板：职能分组（section）+ 细粒度等级过滤（min_class）
/// 说明：旧的 panel 字段（sysop/admin/moderator）是权限等级，曾被当作分组维度，
/// 导致同一职能被拆散；现改为按 section 分组、按 min_class 逐条过滤，无权条目直接不返回。
#[get("/admin/staffpanel")]
async fn staff_panel(
    req: HttpRequest,
    state: web::Data<std::sync::Arc<AppState>>,
) -> DomainResult<HttpResponse> {
    let auth = staff(&req, &state).await?;
    let rows: Vec<StaffPanelEntry> = sqlx::query_as::<_, StaffPanelEntry>(&format!(
        "SELECT section, name, url, info, tab_key, min_class FROM staff_panel_entries \
         WHERE min_class <= $1 AND {} ORDER BY section, sort, id",
        crate::modules::module_on_sql("staff_panel_entries")
    ))
    .bind(auth.class_id)
    .fetch_all(&state.repo.db)
    .await
    .map_err(|e| DomainError::Internal(e.into()))?;
    Ok(ok(serde_json::json!({
        "entries": rows,
        "role": if auth.class_id >= 99 { "sysop" }
            else if auth.class_id >= 93 { "administrator" }
            else { "moderator" },
        "class_id": auth.class_id,
    })))
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
