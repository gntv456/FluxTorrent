//! P2-8 头像挂件 CRUD（CSS 白名单过滤）
//! 从 admin_p3_http.rs 按域拆出。

use actix_web::{delete, get, post, put, web, HttpRequest, HttpResponse};
use serde::Deserialize;
use std::sync::Arc;

use crate::dto::ok;
use crate::errors::{DomainError, DomainResult};
use crate::state::AppState;

use super::staff;

// ============ P2-8 道具 CRUD 与用户背包 ============

// ---- 头像框库 CRUD（avatar_frames：佩戴展示的框样式，四季系列同表）----

#[derive(sqlx::FromRow, serde::Serialize)]
struct AvatarFrameRow {
    id: i32,
    name: String,
    css: String,
    #[sqlx(default)]
    image_url: Option<String>,
    price: i32,
    sort: i32,
    /// 佩戴人数（商店页排序参考；不能物理删佩戴中的框）
    worn_count: i64,
}

/// css 净化：只放行 border-color / box-shadow 声明（与前端 avatarFrameStyle 白名单一致，
/// 防后台误编辑注入无关样式）；全被滤掉时回退默认灰描边，保证框永远可见
fn sanitize_frame_css(css: &str) -> String {
    let kept: Vec<String> = css
        .split(';')
        .filter_map(|decl| {
            let (k, v) = decl.split_once(':')?;
            let (k, v) = (k.trim(), v.trim());
            (matches!(k, "border-color" | "box-shadow") && !v.is_empty())
                .then(|| format!("{k}: {v}"))
        })
        .collect();
    if kept.is_empty() {
        "border-color: #d7dee8; box-shadow: 0 0 0 3px #d7dee8".into()
    } else {
        format!("{};", kept.join("; "))
    }
}

#[get("/admin/avatar-frames")]
async fn admin_avatar_frames(
    req: HttpRequest,
    state: web::Data<std::sync::Arc<AppState>>,
) -> DomainResult<HttpResponse> {
    let _auth = staff(&req, &state).await?;
    let rows: Vec<AvatarFrameRow> = sqlx::query_as(
        "SELECT f.id, f.name, f.css, f.image_url, f.price, f.sort, \
                (SELECT count(*) FROM users u WHERE u.avatar_frame_id = f.id)::bigint AS worn_count \
         FROM avatar_frames f ORDER BY f.sort, f.id",
    )
    .fetch_all(&state.repo.db)
    .await
    .map_err(|e| DomainError::Internal(e.into()))?;
    Ok(ok(serde_json::to_value(rows).unwrap_or_default()))
}

#[derive(Deserialize)]
struct AvatarFrameReq {
    name: String,
    #[serde(default)]
    css: Option<String>,
    /// 框图链接（PNG/GIF 立绘框）；与 css 可共存，同时配置时前端图优先
    #[serde(default)]
    image_url: Option<String>,
    #[serde(default)]
    price: Option<i32>,
    #[serde(default)]
    sort: Option<i32>,
}

/// 图片链接净化：只收 http(s)/协议相对的 URL，去首尾空白；空串归一为 NULL（清图）
fn normalize_frame_image(url: Option<&str>) -> Option<String> {
    let u = url?.trim();
    if u.is_empty() {
        return None;
    }
    let ok = u.starts_with("https://")
        || u.starts_with("http://")
        || u.starts_with("//");
    ok.then(|| u.to_string())
}

#[post("/admin/avatar-frames")]
async fn admin_avatar_frame_add(
    req: HttpRequest,
    state: web::Data<std::sync::Arc<AppState>>,
    body: web::Json<AvatarFrameReq>,
) -> DomainResult<HttpResponse> {
    let auth = staff(&req, &state).await?;
    crate::authz::require_perm(&state, &auth, crate::authz::perm::PROP_MANAGE)
        .await?;
    if body.name.trim().is_empty() {
        return Err(DomainError::Validation("名称必填".into()));
    }
    let css = sanitize_frame_css(body.css.as_deref().unwrap_or(""));
    let image = normalize_frame_image(body.image_url.as_deref());
    let id: i32 = sqlx::query_scalar(
        "INSERT INTO avatar_frames (name, css, image_url, price, sort) \
         VALUES ($1, $2, $3, COALESCE($4, 0), COALESCE($5, 0)) RETURNING id",
    )
    .bind(body.name.trim())
    .bind(&css)
    .bind(&image)
    .bind(body.price)
    .bind(body.sort.unwrap_or(0))
    .fetch_one(&state.repo.db)
    .await
    .map_err(|e| DomainError::Internal(e.into()))?;
    state
        .repo
        .audit(Some(auth.id), "frame.add", Some(id as i64))
        .await;
    Ok(ok(serde_json::json!({ "id": id })))
}

#[put("/admin/avatar-frames/{id}")]
async fn admin_avatar_frame_update(
    req: HttpRequest,
    state: web::Data<Arc<AppState>>,
    path: web::Path<i32>,
    body: web::Json<AvatarFrameReq>,
) -> DomainResult<HttpResponse> {
    let auth = staff(&req, &state).await?;
    crate::authz::require_perm(&state, &auth, crate::authz::perm::PROP_MANAGE)
        .await?;
    let id = path.into_inner();
    if body.name.trim().is_empty() {
        return Err(DomainError::Validation("名称必填".into()));
    }
    let css = sanitize_frame_css(body.css.as_deref().unwrap_or(""));
    let image = normalize_frame_image(body.image_url.as_deref());
    let n = sqlx::query(
        "UPDATE avatar_frames SET name = $2, css = $3, image_url = $4, \
         price = COALESCE($5, price), sort = COALESCE($6, sort) WHERE id = $1",
    )
    .bind(id)
    .bind(body.name.trim())
    .bind(&css)
    .bind(&image)
    .bind(body.price)
    .bind(body.sort)
    .execute(&state.repo.db)
    .await
    .map_err(|e| DomainError::Internal(e.into()))?
    .rows_affected();
    if n == 0 {
        return Err(DomainError::NotFound(id as i64));
    }
    state
        .repo
        .audit(Some(auth.id), "frame.update", Some(id as i64))
        .await;
    Ok(ok(serde_json::json!({ "ok": true })))
}

#[delete("/admin/avatar-frames/{id}")]
async fn admin_avatar_frame_delete(
    req: HttpRequest,
    state: web::Data<Arc<AppState>>,
    path: web::Path<i32>,
) -> DomainResult<HttpResponse> {
    let auth = staff(&req, &state).await?;
    crate::authz::require_perm(&state, &auth, crate::authz::perm::PROP_MANAGE)
        .await?;
    let id = path.into_inner();
    // 佩戴中不允许物理删（users.avatar_frame_id 外键）：先摘下所有佩戴者再删
    sqlx::query(
        "UPDATE users SET avatar_frame_id = NULL WHERE avatar_frame_id = $1",
    )
    .bind(id)
    .execute(&state.repo.db)
    .await
    .map_err(|e| DomainError::Internal(e.into()))?;
    let n = sqlx::query("DELETE FROM avatar_frames WHERE id = $1")
        .bind(id)
        .execute(&state.repo.db)
        .await
        .map_err(|e| DomainError::Internal(e.into()))?
        .rows_affected();
    if n == 0 {
        return Err(DomainError::NotFound(id as i64));
    }
    state
        .repo
        .audit(Some(auth.id), "frame.del", Some(id as i64))
        .await;
    Ok(ok(serde_json::json!({ "deleted": id })))
}
