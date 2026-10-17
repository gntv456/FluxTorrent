use actix_web::{delete, post, put, web, HttpRequest, HttpResponse};
use serde::Deserialize;

use crate::dto::ok;
use crate::errors::{DomainError, DomainResult};
use crate::state::AppState;

use super::forum_check::{
    current_gates, ensure_category_exists, forum_upsert_check,
};
use super::guard::staff;
use crate::repo::forum as repo_forum;

#[derive(Deserialize)]
pub(crate) struct ForumUpsertReq {
    pub(crate) name: String,
    #[serde(default)]
    pub(crate) descr: Option<String>,
    /// 版块规则正文（0392）：缺省=不改，空串=清空（见 repo::forum）
    #[serde(default)]
    pub(crate) rules: Option<String>,
    #[serde(default)]
    pub(crate) minclassread: Option<i32>,
    #[serde(default)]
    pub(crate) minclasswrite: Option<i32>,
    #[serde(default)]
    pub(crate) minclasscreate: Option<i32>,
    #[serde(default)]
    pub(crate) protected: Option<bool>,
    /// 归属分区（0115）；None = 不分组
    #[serde(default)]
    pub(crate) category_id: Option<i64>,
    /// 分区内排序（0154）
    #[serde(default)]
    pub(crate) sort: Option<i32>,
    /// 乐观并发版本戳（FRAME §5 推广项之二 / 0376）：编辑页回填
    /// 版块当前 `mtime`，保存时带上。不匹配即 409，避免两名版主
    /// 并发编辑互相整条覆盖（此前 last-write-wins，三档门槛被静默
    /// 清成 0/0/0 就是这类覆盖的典型后果）。缺省不校验（灰度）。
    #[serde(default)]
    pub(crate) expected_mtime: Option<String>,
}

#[post("/admin/forums")]
async fn forum_admin_create(
    req: HttpRequest,
    state: web::Data<std::sync::Arc<AppState>>,
    body: web::Json<ForumUpsertReq>,
) -> DomainResult<HttpResponse> {
    let auth = staff(&req, &state).await?;
    crate::authz::require_perm(
        &state,
        &auth,
        crate::authz::perm::FORUMS_MANAGE,
    )
    .await?;
    let name = forum_upsert_check(&body)?;
    ensure_category_exists(&state.repo.db, body.category_id).await?;
    let id = repo_forum::create_forum(
        &state.repo,
        repo_forum::ForumCreate {
            name: &name,
            descr: body.descr.as_deref(),
            rules: body.rules.as_deref().map(str::trim),
            mr: body.minclassread.unwrap_or(0),
            mw: body.minclasswrite.unwrap_or(0),
            mc: body.minclasscreate.unwrap_or(0),
            protected: body.protected.unwrap_or(false),
            category_id: body.category_id,
            sort: body.sort.unwrap_or(0),
        },
    )
    .await?;
    state
        .repo
        .audit(Some(auth.id), "forum.create", Some(id))
        .await;
    Ok(ok(serde_json::json!({ "id": id })))
}

#[put("/admin/forums/{id}")]
async fn forum_admin_update(
    req: HttpRequest,
    state: web::Data<std::sync::Arc<AppState>>,
    path: web::Path<i64>,
    body: web::Json<ForumUpsertReq>,
) -> DomainResult<HttpResponse> {
    let auth = staff(&req, &state).await?;
    crate::authz::require_perm(
        &state,
        &auth,
        crate::authz::perm::FORUMS_MANAGE,
    )
    .await?;
    let name = forum_upsert_check(&body)?;
    let fid = path.into_inner();
    ensure_category_exists(&state.repo.db, body.category_id).await?;
    let (mr, mw, mc) = (
        body.minclassread.unwrap_or(0),
        body.minclasswrite.unwrap_or(0),
        body.minclasscreate.unwrap_or(0),
    );
    // 防「静默放宽权限」：编辑表单若不回填，会把三档门槛写成 0/0/0
    //（0 = 所有人可读/回/发）。这里留一条可追溯的审计。
    if let Some((br, bw, bc)) = current_gates(&state.repo.db, fid).await? {
        let was_open = (mr, mw, mc) == (0, 0, 0);
        if (br, bw, bc) != (0, 0, 0) && was_open {
            tracing::warn!(
                forum_id = fid,
                actor = auth.id,
                "版块三档门槛被清空为 0/0/0（权限放宽），请核对是否有意为之"
            );
            state
                .repo
                .audit(Some(auth.id), "forum.gates_cleared", Some(fid))
                .await;
        }
    }
    // 乐观并发（0376）：带 expected_mtime 时把它写进 WHERE 的比对条件，
    // 命中 0 行 = 期间被别人改过 → 409 而不是静默覆盖。非 RFC3339 一律
    // 按冲突处理（宁可让版主重看一次，也不要拿一个坏戳名正言顺地覆盖）。
    // 缺省则退回修前行为（last-write-wins），灰度期前端不改也能上。
    let expected = body.expected_mtime.as_deref().unwrap_or("").trim();
    if !expected.is_empty() {
        // 版本戳读下沉 repo/forum.rs（含 FOR UPDATE，理由见那侧注释）
        let cur = crate::repo::forum::mtime(&state.repo, fid).await;
        let hit = chrono::DateTime::parse_from_rfc3339(expected)
            .map(|t| cur == Some(t.with_timezone(&chrono::Utc)))
            .unwrap_or(false);
        if !hit {
            return Err(DomainError::VersionConflict(vec![fid.to_string()]));
        }
    }
    let n = repo_forum::update_forum(
        &state.repo,
        repo_forum::ForumUpdate {
            name: &name,
            descr: body.descr.as_deref(),
            // 缺省 = 不动这一列；空串 = 版主清空（repo 里分别走 NULL 与 ""）
            rules: body.rules.as_deref().map(str::trim),
            mr,
            mw,
            mc,
            protected: body.protected.unwrap_or(false),
            category_id: body.category_id,
            sort: body.sort.unwrap_or(0),
            fid,
        },
    )
    .await?;
    if n == 0 {
        return Err(DomainError::NotFound(fid));
    }
    state
        .repo
        .audit(Some(auth.id), "forum.update", Some(fid))
        .await;
    Ok(ok(serde_json::json!({ "updated": fid })))
}

#[delete("/admin/forums/{id}")]
async fn forum_admin_delete(
    req: HttpRequest,
    state: web::Data<std::sync::Arc<AppState>>,
    path: web::Path<i64>,
    q: crate::http::Q<std::collections::HashMap<String, String>>,
) -> DomainResult<HttpResponse> {
    let force = q.get("force").map(|v| v == "true").unwrap_or(false);
    let auth = staff(&req, &state).await?;
    crate::authz::require_perm(
        &state,
        &auth,
        crate::authz::perm::FORUMS_MANAGE,
    )
    .await?;
    let fid = path.into_inner();
    // 审计修复（P1）：删除版块会静默级联删除其下全部主题与帖子（topics del=c → posts 级联）。
    // 非空版块要求显式 force=true 才执行，防误删整版内容。
    let topic_cnt: i64 =
        sqlx::query_scalar("SELECT count(*) FROM topics WHERE forum_id = $1")
            .bind(fid)
            .fetch_one(&state.repo.db)
            .await
            .map_err(|e| DomainError::Internal(e.into()))?;
    if topic_cnt > 0 && !force {
        return Err(DomainError::Validation(format!(
            "该版块仍有 {topic_cnt} 个主题（删除将级联清空全部帖子）。确认知悉请传 force=true"
        )));
    }
    let n = sqlx::query("DELETE FROM forums WHERE id = $1")
        .bind(fid)
        .execute(&state.repo.db)
        .await
        .map_err(|e| DomainError::Internal(e.into()))?
        .rows_affected();
    if n == 0 {
        return Err(DomainError::NotFound(fid));
    }
    state
        .repo
        .audit(Some(auth.id), "forum.delete", Some(fid))
        .await;
    Ok(ok(serde_json::json!({ "deleted": fid })))
}

/// 任命版主（无需等级）
#[derive(Deserialize)]
struct ForumModReq {
    username: String,
}

#[post("/admin/forums/{id}/mods")]
async fn forum_mod_add(
    req: HttpRequest,
    state: web::Data<std::sync::Arc<AppState>>,
    path: web::Path<i64>,
    body: web::Json<ForumModReq>,
) -> DomainResult<HttpResponse> {
    let auth = staff(&req, &state).await?;
    crate::authz::require_perm(
        &state,
        &auth,
        crate::authz::perm::FORUMS_MANAGE,
    )
    .await?;
    let fid = path.into_inner();
    super::forum_check::ensure_forum_exists(&state.repo.db, fid).await?;
    let uid: Option<i64> = sqlx::query_scalar(
        "SELECT id FROM users WHERE username = $1 AND status < 2",
    )
    .bind(&body.username)
    .fetch_optional(&state.repo.db)
    .await
    .map_err(|e| DomainError::Internal(e.into()))?;
    let Some(uid) = uid else {
        return Err(DomainError::NotFound(0));
    };
    let added = sqlx::query(
        "INSERT INTO forum_mods (forum_id, user_id, created_by) \
     VALUES ($1, $2, $3) ON CONFLICT DO NOTHING",
    )
    .bind(fid)
    .bind(uid)
    .bind(auth.id)
    .execute(&state.repo.db)
    .await
    .map_err(|e| DomainError::Internal(e.into()))?
    .rows_affected();
    if added == 0 {
        // 已经是这个版块的版主：不能再写审计，也不能回一个「像新建成功」的体
        //（假成功 + 假审计：站长以为多任命了一个人，账上还多一条 mod_add）
        return Ok(ok(serde_json::json!({
            "forum_id": fid, "user_id": uid, "added": false,
            "already": true,
        })));
    }
    state
        .repo
        .audit_detail(
            Some(auth.id),
            "forum.mod_add",
            Some(uid),
            None,
            Some(serde_json::json!({ "forum_id": fid })),
        )
        .await;
    Ok(ok(serde_json::json!(
        { "forum_id": fid, "user_id": uid, "added": true }
    )))
}

#[delete("/admin/forums/{id}/mods/{user_id}")]
async fn forum_mod_remove(
    req: HttpRequest,
    state: web::Data<std::sync::Arc<AppState>>,
    path: web::Path<(i64, i64)>,
) -> DomainResult<HttpResponse> {
    let auth = staff(&req, &state).await?;
    crate::authz::require_perm(
        &state,
        &auth,
        crate::authz::perm::FORUMS_MANAGE,
    )
    .await?;
    let (fid, uid) = path.into_inner();
    let n = sqlx::query(
        "DELETE FROM forum_mods WHERE forum_id = $1 AND user_id = $2",
    )
    .bind(fid)
    .bind(uid)
    .execute(&state.repo.db)
    .await
    .map_err(|e| DomainError::Internal(e.into()))?
    .rows_affected();
    if n == 0 {
        return Err(DomainError::Validation("该版块没有这名版主".into()));
    }
    state
        .repo
        .audit_detail(
            Some(auth.id),
            "forum.mod_remove",
            Some(uid),
            None,
            Some(serde_json::json!({ "forum_id": fid })),
        )
        .await;
    Ok(ok(serde_json::json!({ "removed": uid, "forum_id": fid })))
}
