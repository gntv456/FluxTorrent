//! 第八轮 P3：管理套件（好学站后台逐页深挖对照报告落地）。
//! 种子批量工作台 / 标签字典 / H&R 总览 / 邀请·签到·改名·修改记录 /
//! 勋章·道具 CRUD / 用户批量操作 / Section 多维 / 考核·任务配置 / Tracker URL。

use actix_web::{delete, get, post, put, web, HttpRequest, HttpResponse};
use serde::Deserialize;

use crate::dto::ok;
use crate::errors::{DomainError, DomainResult};
use crate::http::require_auth;
use crate::state::AppState;

const MAX_BATCH: usize = 500;

async fn staff(
    req: &HttpRequest,
    state: &web::Data<std::sync::Arc<AppState>>,
) -> DomainResult<crate::http::AuthUser> {
    let auth = require_auth(req, state).await?;
    crate::authz::require_perm(&state, &auth, crate::authz::perm::STAFF_PANEL).await?;
    Ok(auth)
}

/// 用户修改记录（好学站 UserModifyLog 口径）：管理动作按用户落一条可读摘要
pub async fn modify_log(db: &sqlx::PgPool, uid: i64, modifier: Option<i64>, content: &str) {
    let _ =
        sqlx::query("INSERT INTO user_modify_logs (uid, modifier, content) VALUES ($1, $2, $3)")
            .bind(uid)
            .bind(modifier)
            .bind(content)
            .execute(db)
            .await;
}

fn check_ids(ids: &[i64]) -> DomainResult<()> {
    if ids.is_empty() {
        return Err(DomainError::Validation("缺少目标 ID".into()));
    }
    if ids.len() > MAX_BATCH {
        return Err(DomainError::Validation(format!("单批最多 {MAX_BATCH} 条")));
    }
    Ok(())
}

// ============ P1-1 种子批量工作台（好学站 torrent/torrents 批量动作口径） ============

#[derive(Deserialize)]
struct TorrentBatchReq {
    action: String,
    ids: Vec<i64>,
    /// sticky: 0 普通 / 1 置顶
    #[serde(default)]
    pos_state: Option<i16>,
    #[serde(default)]
    pos_state_until: Option<chrono::DateTime<chrono::Utc>>,
    /// promo: free/x2/x2free/half/x2half/p30
    #[serde(default)]
    promo_kind: Option<String>,
    #[serde(default)]
    promo_until: Option<chrono::DateTime<chrono::Utc>>,
    /// recommend: 0 普通 / 1 推荐 / 2 经典
    #[serde(default)]
    pick_type: Option<i16>,
    #[serde(default)]
    tag_ids: Vec<i32>,
    #[serde(default)]
    category_id: Option<i32>,
    #[serde(default)]
    medium_id: Option<i32>,
    #[serde(default)]
    grade_id: Option<i32>,
    #[serde(default)]
    edition_id: Option<i32>,
    /// 维度字典：kind → dict_id（change_sections 动作）
    #[serde(default)]
    sections: std::collections::HashMap<String, i64>,
}

const PROMO_KINDS: [&str; 6] = ["free", "x2", "x2free", "half", "x2half", "p30"];

#[post("/admin/torrents/batch")]
async fn torrent_batch(
    req: HttpRequest,
    state: web::Data<std::sync::Arc<AppState>>,
    body: web::Json<TorrentBatchReq>,
) -> DomainResult<HttpResponse> {
    let auth = staff(&req, &state).await?;
    crate::authz::require_perm(&state, &auth, crate::authz::perm::TORRENT_MANAGE).await?;
    let ids = &body.ids;
    check_ids(ids)?;
    let db = &state.repo.db;
    let id_arr = ids.to_vec();
    let n: u64 = match body.action.as_str() {
        "sticky" => {
            let ps = body.pos_state.unwrap_or(1);
            if !(0..=1).contains(&ps) {
                return Err(DomainError::Validation("pos_state 取值 0/1".into()));
            }
            sqlx::query(
                "UPDATE torrents SET pos_state = $2, pos_state_until = $3, mtime = now() \
                 WHERE id = ANY($1)",
            )
            .bind(&id_arr)
            .bind(ps)
            .bind(body.pos_state_until)
            .execute(db)
            .await
            .map_err(|e| DomainError::Internal(e.into()))?
            .rows_affected()
        }
        "promo" => {
            let kind = body.promo_kind.as_deref().unwrap_or("free");
            if !PROMO_KINDS.contains(&kind) {
                return Err(DomainError::Validation("未知促销类型".into()));
            }
            let until = body
                .promo_until
                .unwrap_or_else(|| chrono::Utc::now() + chrono::Duration::hours(48));
            sqlx::query(
                "DELETE FROM promotions WHERE scope = 'torrent' AND torrent_id = ANY($1) AND source = 'manual'",
            )
            .bind(&id_arr)
            .execute(db)
            .await
            .map_err(|e| DomainError::Internal(e.into()))?;
            sqlx::query(
                "INSERT INTO promotions (scope, torrent_id, kind, starts_at, ends_at, source, created_by) \
                 SELECT 'torrent', id, $2::promotion_kind_enum, now(), $3, 'manual', $4 \
                 FROM torrents WHERE id = ANY($1)",
            )
            .bind(&id_arr)
            .bind(kind)
            .bind(until)
            .bind(auth.id)
            .execute(db)
            .await
            .map_err(|e| DomainError::Internal(e.into()))?
            .rows_affected()
        }
        "recommend" => {
            let pt = body.pick_type.unwrap_or(0);
            if !(0..=2).contains(&pt) {
                return Err(DomainError::Validation("pick_type 取值 0/1/2".into()));
            }
            sqlx::query("UPDATE torrents SET pick_type = $2, mtime = now() WHERE id = ANY($1)")
                .bind(&id_arr)
                .bind(pt)
                .execute(db)
                .await
                .map_err(|e| DomainError::Internal(e.into()))?
                .rows_affected()
        }
        "set_tags" => {
            if body.tag_ids.is_empty() {
                return Err(DomainError::Validation("缺少标签".into()));
            }
            sqlx::query(
                "INSERT INTO tags (torrent_id, tag_id) \
                 SELECT t.id, tg FROM torrents t, unnest($2::int[]) tg \
                 WHERE t.id = ANY($1) ON CONFLICT DO NOTHING",
            )
            .bind(&id_arr)
            .bind(&body.tag_ids)
            .execute(db)
            .await
            .map_err(|e| DomainError::Internal(e.into()))?
            .rows_affected()
        }
        "clear_tags" => sqlx::query(
            "DELETE FROM tags WHERE torrent_id = ANY($1) \
             AND ($2::int[] IS NULL OR tag_id = ANY($2))",
        )
        .bind(&id_arr)
        .bind(if body.tag_ids.is_empty() {
            None
        } else {
            Some(body.tag_ids.clone())
        })
        .execute(db)
        .await
        .map_err(|e| DomainError::Internal(e.into()))?
        .rows_affected(),
        "hr" | "unhr" => {
            let on = body.action == "hr";
            sqlx::query("UPDATE torrents SET hr_policy = $2, mtime = now() WHERE id = ANY($1)")
                .bind(&id_arr)
                .bind(if on {
                    serde_json::json!({ "on": true })
                } else {
                    serde_json::Value::Null
                })
                .execute(db)
                .await
                .map_err(|e| DomainError::Internal(e.into()))?
                .rows_affected()
        }
        "change_category" => sqlx::query(
            "UPDATE torrents SET \
               category_id = COALESCE($2, category_id), \
               medium_id = COALESCE($3, medium_id), \
               grade_id = COALESCE($4, grade_id), \
               edition_id = COALESCE($5, edition_id), mtime = now() \
             WHERE id = ANY($1)",
        )
        .bind(&id_arr)
        .bind(body.category_id)
        .bind(body.medium_id)
        .bind(body.grade_id)
        .bind(body.edition_id)
        .execute(db)
        .await
        .map_err(|e| DomainError::Internal(e.into()))?
        .rows_affected(),
        "change_sections" => {
            if body.sections.is_empty() {
                return Err(DomainError::Validation("缺少维度取值".into()));
            }
            let mut n: u64 = 0;
            for (kind, dict_id) in &body.sections {
                n += sqlx::query(
                    "INSERT INTO torrent_sections (torrent_id, kind, dict_id) \
                     SELECT id, $2, $3 FROM torrents WHERE id = ANY($1) \
                     ON CONFLICT (torrent_id, kind) DO UPDATE SET dict_id = EXCLUDED.dict_id",
                )
                .bind(&id_arr)
                .bind(kind)
                .bind(dict_id)
                .execute(db)
                .await
                .map_err(|e| DomainError::Internal(e.into()))?
                .rows_affected();
            }
            n
        }
        "delete" => {
            sqlx::query("UPDATE torrents SET approval_status = 3, mtime = now() WHERE id = ANY($1)")
                .bind(&id_arr)
                .execute(db)
                .await
                .map_err(|e| DomainError::Internal(e.into()))?
                .rows_affected()
        }
        _ => return Err(DomainError::Validation("未知批量动作".into())),
    };
    // 操作记录：每种动作写一条汇总（detail 带 ids），列表页可按种子过滤
    sqlx::query(
        "INSERT INTO torrent_operation_logs (torrent_id, operator_id, action, detail) \
         SELECT id, $2, $3, $4 FROM torrents WHERE id = ANY($1)",
    )
    .bind(&id_arr)
    .bind(auth.id)
    .bind(format!("batch_{}", body.action))
    .bind(serde_json::json!({ "ids": ids.len(), "action": body.action }))
    .execute(db)
    .await
    .map_err(|e| DomainError::Internal(e.into()))?;
    state
        .repo
        .audit(
            Some(auth.id),
            &format!("torrent.batch.{}", body.action),
            None,
        )
        .await;
    Ok(ok(serde_json::json!({ "affected": n as i64 })))
}

// ============ P1-2 标签字典（好学站 tags 口径：名称 + 样式属性 + 作用域） ============

#[derive(sqlx::FromRow, serde::Serialize)]
struct TagDictRow {
    id: i32,
    name: String,
    kind: String,
    bg_color: String,
    color: String,
    font_size: String,
    margin: String,
    padding: String,
    border_radius: String,
    sort: i32,
    enabled: bool,
    mode_id: Option<i32>,
}

#[derive(Deserialize)]
struct TagDictReq {
    name: String,
    #[serde(default)]
    kind: Option<String>,
    #[serde(default)]
    bg_color: Option<String>,
    #[serde(default)]
    color: Option<String>,
    #[serde(default)]
    font_size: Option<String>,
    #[serde(default)]
    margin: Option<String>,
    #[serde(default)]
    padding: Option<String>,
    #[serde(default)]
    border_radius: Option<String>,
    #[serde(default)]
    sort: Option<i32>,
    #[serde(default)]
    enabled: Option<bool>,
    #[serde(default)]
    mode_id: Option<i32>,
}

#[get("/admin/tags-dict")]
async fn tags_dict_list(
    req: HttpRequest,
    state: web::Data<std::sync::Arc<AppState>>,
) -> DomainResult<HttpResponse> {
    let auth = staff(&req, &state).await?;
    crate::authz::require_perm(&state, &auth, crate::authz::perm::CATEGORIES_MANAGE).await?;
    let rows: Vec<TagDictRow> = sqlx::query_as(
        "SELECT id, name, kind, bg_color, color, font_size, margin, padding, border_radius, sort, enabled, mode_id \
         FROM tag_dict ORDER BY sort, id",
    )
    .fetch_all(&state.repo.db)
    .await
    .map_err(|e| DomainError::Internal(e.into()))?;
    Ok(ok(serde_json::to_value(rows).unwrap_or_default()))
}

#[post("/admin/tags-dict")]
async fn tags_dict_add(
    req: HttpRequest,
    state: web::Data<std::sync::Arc<AppState>>,
    body: web::Json<TagDictReq>,
) -> DomainResult<HttpResponse> {
    let auth = staff(&req, &state).await?;
    crate::authz::require_perm(&state, &auth, crate::authz::perm::CATEGORIES_MANAGE).await?;
    let name = body.name.trim();
    if name.is_empty() {
        return Err(DomainError::Validation("标签名不能为空".into()));
    }
    // tag_dict.name 有唯一约束（0063）：预查重，撞键时给校验错误而非 500
    let dup: bool = sqlx::query_scalar("SELECT EXISTS(SELECT 1 FROM tag_dict WHERE name = $1)")
        .bind(name)
        .fetch_one(&state.repo.db)
        .await
        .unwrap_or(false);
    if dup {
        return Err(DomainError::Validation("同名标签已存在".into()));
    }
    let next: i32 = sqlx::query_scalar("SELECT COALESCE(max(id), 0) + 1 FROM tag_dict")
        .fetch_one(&state.repo.db)
        .await
        .map_err(|e| DomainError::Internal(e.into()))?;
    sqlx::query(
        "INSERT INTO tag_dict (id, name, kind, bg_color, color, font_size, margin, padding, border_radius, sort, enabled, mode_id) \
         VALUES ($1, $2, COALESCE($3, 'plain'), COALESCE($4, ''), COALESCE($5, '#ffffff'), \
                 COALESCE($6, '12px'), COALESCE($7, '0 4px 0 0'), COALESCE($8, '1px 4px'), \
                 COALESCE($9, '2px'), COALESCE($10, 0), COALESCE($11, TRUE), $12)",
    )
    .bind(next)
    .bind(name)
    .bind(body.kind.clone())
    .bind(body.bg_color.clone())
    .bind(body.color.clone())
    .bind(body.font_size.clone())
    .bind(body.margin.clone())
    .bind(body.padding.clone())
    .bind(body.border_radius.clone())
    .bind(body.sort)
    .bind(body.enabled)
    .bind(body.mode_id)
    .execute(&state.repo.db)
    .await
    .map_err(|e| DomainError::Internal(e.into()))?;
    state
        .repo
        .audit(Some(auth.id), "tagdict.add", Some(next as i64))
        .await;
    Ok(ok(serde_json::json!({ "id": next })))
}

#[put("/admin/tags-dict/{id}")]
async fn tags_dict_update(
    req: HttpRequest,
    state: web::Data<std::sync::Arc<AppState>>,
    path: web::Path<i32>,
    body: web::Json<TagDictReq>,
) -> DomainResult<HttpResponse> {
    let auth = staff(&req, &state).await?;
    crate::authz::require_perm(&state, &auth, crate::authz::perm::CATEGORIES_MANAGE).await?;
    let id = path.into_inner();
    let n = sqlx::query(
        "UPDATE tag_dict SET name = $2, kind = COALESCE($3, kind), bg_color = COALESCE($4, bg_color), \
           color = COALESCE($5, color), font_size = COALESCE($6, font_size), margin = COALESCE($7, margin), \
           padding = COALESCE($8, padding), border_radius = COALESCE($9, border_radius), \
           sort = COALESCE($10, sort), enabled = COALESCE($11, enabled), mode_id = $12 \
         WHERE id = $1",
    )
    .bind(id)
    .bind(body.name.trim())
    .bind(body.kind.clone())
    .bind(body.bg_color.clone())
    .bind(body.color.clone())
    .bind(body.font_size.clone())
    .bind(body.margin.clone())
    .bind(body.padding.clone())
    .bind(body.border_radius.clone())
    .bind(body.sort)
    .bind(body.enabled)
    .bind(body.mode_id)
    .execute(&state.repo.db)
    .await
    .map_err(|e| DomainError::Internal(e.into()))?
    .rows_affected();
    if n == 0 {
        return Err(DomainError::NotFound(id as i64));
    }
    state
        .repo
        .audit(Some(auth.id), "tagdict.update", Some(id as i64))
        .await;
    Ok(ok(serde_json::json!({ "ok": true })))
}

#[delete("/admin/tags-dict/{id}")]
async fn tags_dict_delete(
    req: HttpRequest,
    state: web::Data<std::sync::Arc<AppState>>,
    path: web::Path<i32>,
) -> DomainResult<HttpResponse> {
    let auth = staff(&req, &state).await?;
    crate::authz::require_perm(&state, &auth, crate::authz::perm::CATEGORIES_MANAGE).await?;
    let id = path.into_inner();
    sqlx::query("DELETE FROM tags WHERE tag_id = $1")
        .bind(id)
        .execute(&state.repo.db)
        .await
        .map_err(|e| DomainError::Internal(e.into()))?;
    let n = sqlx::query("DELETE FROM tag_dict WHERE id = $1")
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
        .audit(Some(auth.id), "tagdict.del", Some(id as i64))
        .await;
    Ok(ok(serde_json::json!({ "deleted": id })))
}

// ============ P1-3 H&R 总览（好学站 user/hit-and-runs 口径） ============

#[derive(sqlx::FromRow, serde::Serialize)]
struct HrRecordRow {
    user_id: i64,
    username: String,
    torrent_id: i64,
    torrent_name: Option<String>,
    uploaded: i64,
    downloaded: i64,
    required_seconds: i32,
    seeded_seconds: i32,
    deadline: chrono::DateTime<chrono::Utc>,
    status: String,
    pardoned_name: Option<String>,
    updated_at: chrono::DateTime<chrono::Utc>,
}

#[derive(Deserialize)]
struct HrListQ {
    #[serde(default)]
    uid: Option<i64>,
    #[serde(default)]
    status: Option<String>,
    #[serde(default = "crate::admin_http::default_page")]
    page: i64,
    #[serde(default = "crate::admin_http::default_per_page")]
    per_page: i64,
}

#[get("/admin/hr/records")]
async fn hr_records(
    req: HttpRequest,
    state: web::Data<std::sync::Arc<AppState>>,
    q: web::Query<HrListQ>,
) -> DomainResult<HttpResponse> {
    let auth = staff(&req, &state).await?;
    crate::authz::require_perm(&state, &auth, crate::authz::perm::HR_VIEW).await?;
    if !(1..=100).contains(&q.per_page) {
        return Err(DomainError::Validation("per_page 取值 1-100".into()));
    }
    let rows: Vec<HrRecordRow> = sqlx::query_as(
        r#"SELECT s.user_id, u.username, s.torrent_id, t.name AS torrent_name,
                  s2.uploaded, s2.downloaded, s.required_seconds, s.seeded_seconds,
                  s.deadline, s.status, p.username AS pardoned_name, s.updated_at
           FROM hr_snapshots s
           JOIN users u ON u.id = s.user_id
           JOIN torrents t ON t.id = s.torrent_id
           LEFT JOIN snatches s2 ON s2.user_id = s.user_id AND s2.torrent_id = s.torrent_id
           LEFT JOIN users p ON p.id = s.pardoned_by
           WHERE ($1::bigint IS NULL OR s.user_id = $1)
             AND ($2::text IS NULL OR s.status = $2)
           ORDER BY s.updated_at DESC LIMIT $3 OFFSET $4"#,
    )
    .bind(q.uid)
    .bind(q.status.as_deref().filter(|s| !s.is_empty()))
    .bind(q.per_page)
    .bind((q.page.max(1) - 1) * q.per_page)
    .fetch_all(&state.repo.db)
    .await
    .map_err(|e| DomainError::Internal(e.into()))?;
    let total: i64 = sqlx::query_scalar(
        "SELECT count(*) FROM hr_snapshots s WHERE ($1::bigint IS NULL OR s.user_id = $1) \
         AND ($2::text IS NULL OR s.status = $2)",
    )
    .bind(q.uid)
    .bind(q.status.as_deref().filter(|s| !s.is_empty()))
    .fetch_one(&state.repo.db)
    .await
    .unwrap_or(0);
    Ok(ok(
        serde_json::json!({ "rows": rows, "total": total, "page": q.page.max(1), "per_page": q.per_page }),
    ))
}

#[derive(Deserialize)]
struct HrPardonItem {
    user_id: i64,
    torrent_id: i64,
}

#[derive(Deserialize)]
struct HrBatchPardonReq {
    records: Vec<HrPardonItem>,
    note: String,
}

/// 批量豁免：violated → pardoned（复用单条 hr_pardon 语义）
#[post("/admin/hr/batch-pardon")]
async fn hr_batch_pardon(
    req: HttpRequest,
    state: web::Data<std::sync::Arc<AppState>>,
    body: web::Json<HrBatchPardonReq>,
) -> DomainResult<HttpResponse> {
    let auth = staff(&req, &state).await?;
    crate::authz::require_perm(&state, &auth, crate::authz::perm::HR_PARDON).await?;
    if body.note.trim().is_empty() {
        return Err(DomainError::Validation("豁免必须填理由".into()));
    }
    check_ids(
        &body
            .records
            .iter()
            .map(|r| r.torrent_id)
            .collect::<Vec<_>>(),
    )?;
    let db = &state.repo.db;
    let mut pardoned: u64 = 0;
    for r in &body.records {
        let n = sqlx::query(
            "UPDATE hr_snapshots SET status = 'pardoned', pardoned_by = $1, updated_at = now() \
             WHERE user_id = $2 AND torrent_id = $3 AND status IN ('violated','open')",
        )
        .bind(auth.id)
        .bind(r.user_id)
        .bind(r.torrent_id)
        .execute(db)
        .await
        .map_err(|e| DomainError::Internal(e.into()))?
        .rows_affected();
        if n > 0 {
            pardoned += n;
            sqlx::query(
                "UPDATE hr_violations SET resolved_at = now(), resolved_by = $1 \
                 WHERE user_id = $2 AND torrent_id = $3 AND resolved_at IS NULL",
            )
            .bind(auth.id)
            .bind(r.user_id)
            .bind(r.torrent_id)
            .execute(db)
            .await
            .map_err(|e| DomainError::Internal(e.into()))?;
            state
                .repo
                .audit(Some(auth.id), "hr.batch_pardon", Some(r.torrent_id))
                .await;
        }
    }
    Ok(ok(serde_json::json!({ "pardoned": pardoned as i64 })))
}

// ============ P2-4 邀请管理（好学站 user/invites 口径） ============

#[derive(sqlx::FromRow, serde::Serialize)]
struct InviteAdminRow {
    id: i64,
    inviter: String,
    inviter_id: i64,
    code: String,
    status: i16,
    used_by: Option<i64>,
    used_by_name: Option<String>,
    expires_at: chrono::DateTime<chrono::Utc>,
}

#[derive(Deserialize)]
struct InviteListQ {
    #[serde(default)]
    uid: Option<i64>,
    /// 0未用 1已用 2过期 3撤销；缺省全部
    #[serde(default)]
    valid: Option<i16>,
    #[serde(default = "crate::admin_http::default_page")]
    page: i64,
    #[serde(default = "crate::admin_http::default_per_page")]
    per_page: i64,
}

#[get("/admin/invites")]
async fn admin_invites(
    req: HttpRequest,
    state: web::Data<std::sync::Arc<AppState>>,
    q: web::Query<InviteListQ>,
) -> DomainResult<HttpResponse> {
    let auth = staff(&req, &state).await?;
    crate::authz::require_perm(&state, &auth, crate::authz::perm::INVITE_VIEW).await?;
    if !(1..=100).contains(&q.per_page) {
        return Err(DomainError::Validation("per_page 取值 1-100".into()));
    }
    let rows: Vec<InviteAdminRow> = sqlx::query_as(
        r#"SELECT i.id, u.username AS inviter, i.inviter_id, i.code, i.status,
                  i.used_by, uu.username AS used_by_name, i.expires_at
           FROM invites i
           JOIN users u ON u.id = i.inviter_id
           LEFT JOIN users uu ON uu.id = i.used_by
           WHERE ($1::bigint IS NULL OR i.inviter_id = $1)
             AND ($2::smallint IS NULL OR i.status = $2)
           ORDER BY i.id DESC LIMIT $3 OFFSET $4"#,
    )
    .bind(q.uid)
    .bind(q.valid)
    .bind(q.per_page)
    .bind((q.page.max(1) - 1) * q.per_page)
    .fetch_all(&state.repo.db)
    .await
    .map_err(|e| DomainError::Internal(e.into()))?;
    let total: i64 = sqlx::query_scalar(
        "SELECT count(*) FROM invites i WHERE ($1::bigint IS NULL OR i.inviter_id = $1) \
         AND ($2::smallint IS NULL OR i.status = $2)",
    )
    .bind(q.uid)
    .bind(q.valid)
    .fetch_one(&state.repo.db)
    .await
    .unwrap_or(0);
    Ok(ok(
        serde_json::json!({ "rows": rows, "total": total, "page": q.page.max(1), "per_page": q.per_page }),
    ))
}

// ============ P2-5 签到记录与补签 ============

#[derive(sqlx::FromRow, serde::Serialize)]
struct AttendanceRow {
    user_id: i64,
    username: String,
    date: chrono::NaiveDate,
    streak: i32,
    reward: i64,
    makeup: bool,
}

#[derive(Deserialize)]
struct AttendanceQ {
    #[serde(default)]
    uid: Option<i64>,
    #[serde(default)]
    makeup: Option<bool>,
    #[serde(default = "crate::admin_http::default_page")]
    page: i64,
    #[serde(default = "crate::admin_http::default_per_page")]
    per_page: i64,
}

#[get("/admin/attendance")]
async fn admin_attendance(
    req: HttpRequest,
    state: web::Data<std::sync::Arc<AppState>>,
    q: web::Query<AttendanceQ>,
) -> DomainResult<HttpResponse> {
    let auth = staff(&req, &state).await?;
    crate::authz::require_perm(&state, &auth, crate::authz::perm::ATTENDANCE_MANAGE).await?;
    if !(1..=100).contains(&q.per_page) {
        return Err(DomainError::Validation("per_page 取值 1-100".into()));
    }
    let rows: Vec<AttendanceRow> = sqlx::query_as(
        r#"SELECT a.user_id, u.username, a.date, a.streak, a.reward, a.makeup
           FROM attendance a JOIN users u ON u.id = a.user_id
           WHERE ($1::bigint IS NULL OR a.user_id = $1)
             AND ($2::bool IS NULL OR a.makeup = $2)
           ORDER BY a.date DESC, a.user_id LIMIT $3 OFFSET $4"#,
    )
    .bind(q.uid)
    .bind(q.makeup)
    .bind(q.per_page)
    .bind((q.page.max(1) - 1) * q.per_page)
    .fetch_all(&state.repo.db)
    .await
    .map_err(|e| DomainError::Internal(e.into()))?;
    let total: i64 = sqlx::query_scalar(
        "SELECT count(*) FROM attendance a WHERE ($1::bigint IS NULL OR a.user_id = $1) \
         AND ($2::bool IS NULL OR a.makeup = $2)",
    )
    .bind(q.uid)
    .bind(q.makeup)
    .fetch_one(&state.repo.db)
    .await
    .unwrap_or(0);
    Ok(ok(
        serde_json::json!({ "rows": rows, "total": total, "page": q.page.max(1), "per_page": q.per_page }),
    ))
}

#[derive(Deserialize)]
struct MakeupReq {
    user_id: i64,
    date: chrono::NaiveDate,
}

/// 手工补签（补签卡口径）：makeup=true 落一条流水，已签日期跳过
#[post("/admin/attendance/makeup")]
async fn admin_attendance_makeup(
    req: HttpRequest,
    state: web::Data<std::sync::Arc<AppState>>,
    body: web::Json<MakeupReq>,
) -> DomainResult<HttpResponse> {
    let auth = staff(&req, &state).await?;
    crate::authz::require_perm(&state, &auth, crate::authz::perm::ATTENDANCE_MANAGE).await?;
    let exists: bool = sqlx::query_scalar("SELECT EXISTS(SELECT 1 FROM users WHERE id = $1)")
        .bind(body.user_id)
        .fetch_one(&state.repo.db)
        .await
        .map_err(|e| DomainError::Internal(e.into()))?;
    if !exists {
        return Err(DomainError::NotFound(body.user_id));
    }
    let n = sqlx::query(
        "INSERT INTO attendance (user_id, date, streak, reward, makeup) \
         VALUES ($1, $2, 0, 0, TRUE) ON CONFLICT (user_id, date) DO NOTHING",
    )
    .bind(body.user_id)
    .bind(body.date)
    .execute(&state.repo.db)
    .await
    .map_err(|e| DomainError::Internal(e.into()))?
    .rows_affected();
    if n == 0 {
        return Err(DomainError::Validation("该用户当日已有签到记录".into()));
    }
    state
        .repo
        .audit(Some(auth.id), "attendance.makeup", Some(body.user_id))
        .await;
    modify_log(
        &state.repo.db,
        body.user_id,
        Some(auth.id),
        &format!("管理补签 {}", body.date),
    )
    .await;
    Ok(ok(serde_json::json!({ "ok": true })))
}

/// 撤销补签（仅可撤 makeup 行，正常签到不可删）
#[delete("/admin/attendance/makeup")]
async fn admin_attendance_makeup_delete(
    req: HttpRequest,
    state: web::Data<std::sync::Arc<AppState>>,
    body: web::Json<MakeupReq>,
) -> DomainResult<HttpResponse> {
    let auth = staff(&req, &state).await?;
    crate::authz::require_perm(&state, &auth, crate::authz::perm::ATTENDANCE_MANAGE).await?;
    let n =
        sqlx::query("DELETE FROM attendance WHERE user_id = $1 AND date = $2 AND makeup = TRUE")
            .bind(body.user_id)
            .bind(body.date)
            .execute(&state.repo.db)
            .await
            .map_err(|e| DomainError::Internal(e.into()))?
            .rows_affected();
    if n == 0 {
        return Err(DomainError::Validation("无对应补签记录".into()));
    }
    state
        .repo
        .audit(Some(auth.id), "attendance.makeup_del", Some(body.user_id))
        .await;
    Ok(ok(serde_json::json!({ "ok": true })))
}

// ============ P2-6 改名记录 / 用户修改记录 ============

#[derive(sqlx::FromRow, serde::Serialize)]
struct RenameLogRow {
    id: i64,
    uid: i64,
    username: String,
    old_name: String,
    new_name: String,
    operator: Option<i64>,
    operator_name: Option<String>,
    created_at: chrono::DateTime<chrono::Utc>,
}

#[derive(Deserialize)]
struct UidPageQ {
    #[serde(default)]
    uid: Option<i64>,
    #[serde(default = "crate::admin_http::default_page")]
    page: i64,
    #[serde(default = "crate::admin_http::default_per_page")]
    per_page: i64,
}

#[get("/admin/rename-logs")]
async fn admin_rename_logs(
    req: HttpRequest,
    state: web::Data<std::sync::Arc<AppState>>,
    q: web::Query<UidPageQ>,
) -> DomainResult<HttpResponse> {
    let _auth = staff(&req, &state).await?;
    if !(1..=100).contains(&q.per_page) {
        return Err(DomainError::Validation("per_page 取值 1-100".into()));
    }
    let rows: Vec<RenameLogRow> = sqlx::query_as(
        r#"SELECT l.id, l.uid, u.username, l.old_name, l.new_name,
                  l.operator, op.username AS operator_name, l.created_at
           FROM username_change_logs l
           JOIN users u ON u.id = l.uid
           LEFT JOIN users op ON op.id = l.operator
           WHERE ($1::bigint IS NULL OR l.uid = $1)
           ORDER BY l.id DESC LIMIT $2 OFFSET $3"#,
    )
    .bind(q.uid)
    .bind(q.per_page)
    .bind((q.page.max(1) - 1) * q.per_page)
    .fetch_all(&state.repo.db)
    .await
    .map_err(|e| DomainError::Internal(e.into()))?;
    let total: i64 = sqlx::query_scalar(
        "SELECT count(*) FROM username_change_logs WHERE ($1::bigint IS NULL OR uid = $1)",
    )
    .bind(q.uid)
    .fetch_one(&state.repo.db)
    .await
    .unwrap_or(0);
    Ok(ok(
        serde_json::json!({ "rows": rows, "total": total, "page": q.page.max(1), "per_page": q.per_page }),
    ))
}

#[derive(sqlx::FromRow, serde::Serialize)]
struct ModifyLogRow {
    id: i64,
    uid: i64,
    username: String,
    modifier: Option<i64>,
    modifier_name: Option<String>,
    content: String,
    created_at: chrono::DateTime<chrono::Utc>,
}

#[get("/admin/modify-logs")]
async fn admin_modify_logs(
    req: HttpRequest,
    state: web::Data<std::sync::Arc<AppState>>,
    q: web::Query<UidPageQ>,
) -> DomainResult<HttpResponse> {
    let _auth = staff(&req, &state).await?;
    if !(1..=100).contains(&q.per_page) {
        return Err(DomainError::Validation("per_page 取值 1-100".into()));
    }
    let rows: Vec<ModifyLogRow> = sqlx::query_as(
        r#"SELECT l.id, l.uid, u.username, l.modifier, op.username AS modifier_name,
                  l.content, l.created_at
           FROM user_modify_logs l
           JOIN users u ON u.id = l.uid
           LEFT JOIN users op ON op.id = l.modifier
           WHERE ($1::bigint IS NULL OR l.uid = $1)
           ORDER BY l.id DESC LIMIT $2 OFFSET $3"#,
    )
    .bind(q.uid)
    .bind(q.per_page)
    .bind((q.page.max(1) - 1) * q.per_page)
    .fetch_all(&state.repo.db)
    .await
    .map_err(|e| DomainError::Internal(e.into()))?;
    let total: i64 = sqlx::query_scalar(
        "SELECT count(*) FROM user_modify_logs WHERE ($1::bigint IS NULL OR uid = $1)",
    )
    .bind(q.uid)
    .fetch_one(&state.repo.db)
    .await
    .unwrap_or(0);
    Ok(ok(
        serde_json::json!({ "rows": rows, "total": total, "page": q.page.max(1), "per_page": q.per_page }),
    ))
}

// ============ P2-7 勋章 CRUD 与持有管理 ============

#[derive(sqlx::FromRow, serde::Serialize)]
struct MedalAdminRow {
    id: i64,
    name: String,
    description: Option<String>,
    price: Option<i64>,
    rarity: Option<String>,
    limited: bool,
    get_type: i16,
    duration_days: Option<i32>,
    bonus_addition_factor: Option<f64>,
    category_id: i32,
    asset_ref: Option<String>,
    held_count: i64,
}

#[get("/admin/medals")]
async fn admin_medals(
    req: HttpRequest,
    state: web::Data<std::sync::Arc<AppState>>,
) -> DomainResult<HttpResponse> {
    let _auth = staff(&req, &state).await?;
    let rows: Vec<MedalAdminRow> = sqlx::query_as(
        r#"SELECT m.id, m.name, m.description, m.price, m.rarity, m.limited, m.get_type,
                  m.duration_days, m.bonus_addition_factor::float8, m.category_id, m.asset_ref,
                  (SELECT count(*) FROM user_medals um WHERE um.medal_id = m.id)::bigint AS held_count
           FROM medals m ORDER BY m.category_id, m.id"#,
    )
    .fetch_all(&state.repo.db)
    .await
    .map_err(|e| DomainError::Internal(e.into()))?;
    Ok(ok(serde_json::to_value(rows).unwrap_or_default()))
}

#[derive(Deserialize)]
struct MedalReq {
    name: String,
    #[serde(default)]
    description: Option<String>,
    #[serde(default)]
    price: Option<i64>,
    #[serde(default)]
    rarity: Option<String>,
    #[serde(default)]
    limited: Option<bool>,
    #[serde(default)]
    get_type: Option<i16>,
    #[serde(default)]
    duration_days: Option<i32>,
    #[serde(default)]
    bonus_addition_factor: Option<f64>,
    #[serde(default)]
    category_id: Option<i32>,
    #[serde(default)]
    asset_ref: Option<String>,
}

#[post("/admin/medals")]
async fn admin_medal_add(
    req: HttpRequest,
    state: web::Data<std::sync::Arc<AppState>>,
    body: web::Json<MedalReq>,
) -> DomainResult<HttpResponse> {
    let auth = staff(&req, &state).await?;
    crate::authz::require_perm(&state, &auth, crate::authz::perm::MEDAL_MANAGE).await?;
    if body.name.trim().is_empty() {
        return Err(DomainError::Validation("勋章名不能为空".into()));
    }
    let id: i64 = sqlx::query_scalar(
        "INSERT INTO medals (name, description, price, rarity, limited, get_type, duration_days, bonus_addition_factor, category_id, asset_ref) \
         VALUES ($1, $2, $3, $4, COALESCE($5, FALSE), COALESCE($6, 2), $7, $8::numeric, COALESCE($9, 0), $10) RETURNING id",
    )
    .bind(body.name.trim())
    .bind(body.description.clone())
    .bind(body.price)
    .bind(body.rarity.clone())
    .bind(body.limited)
    .bind(body.get_type)
    .bind(body.duration_days)
    .bind(body.bonus_addition_factor)
    .bind(body.category_id)
    .bind(body.asset_ref.clone())
    .fetch_one(&state.repo.db)
    .await
    .map_err(|e| DomainError::Internal(e.into()))?;
    state.repo.audit(Some(auth.id), "medal.add", Some(id)).await;
    Ok(ok(serde_json::json!({ "id": id })))
}

#[put("/admin/medals/{id}")]
async fn admin_medal_update(
    req: HttpRequest,
    state: web::Data<std::sync::Arc<AppState>>,
    path: web::Path<i64>,
    body: web::Json<MedalReq>,
) -> DomainResult<HttpResponse> {
    let auth = staff(&req, &state).await?;
    crate::authz::require_perm(&state, &auth, crate::authz::perm::MEDAL_MANAGE).await?;
    let id = path.into_inner();
    let n = sqlx::query(
        "UPDATE medals SET name = $2, description = $3, price = $4, rarity = $5, limited = COALESCE($6, limited), \
           get_type = COALESCE($7, get_type), duration_days = $8, bonus_addition_factor = COALESCE($9::numeric, bonus_addition_factor), \
           category_id = COALESCE($10, category_id), asset_ref = $11 WHERE id = $1",
    )
    .bind(id)
    .bind(body.name.trim())
    .bind(body.description.clone())
    .bind(body.price)
    .bind(body.rarity.clone())
    .bind(body.limited)
    .bind(body.get_type)
    .bind(body.duration_days)
    .bind(body.bonus_addition_factor)
    .bind(body.category_id)
    .bind(body.asset_ref.clone())
    .execute(&state.repo.db)
    .await
    .map_err(|e| DomainError::Internal(e.into()))?
    .rows_affected();
    if n == 0 {
        return Err(DomainError::NotFound(id));
    }
    state
        .repo
        .audit(Some(auth.id), "medal.update", Some(id))
        .await;
    Ok(ok(serde_json::json!({ "ok": true })))
}

#[delete("/admin/medals/{id}")]
async fn admin_medal_delete(
    req: HttpRequest,
    state: web::Data<std::sync::Arc<AppState>>,
    path: web::Path<i64>,
) -> DomainResult<HttpResponse> {
    let auth = staff(&req, &state).await?;
    crate::authz::require_perm(&state, &auth, crate::authz::perm::MEDAL_MANAGE).await?;
    let id = path.into_inner();
    let n = sqlx::query("DELETE FROM medals WHERE id = $1")
        .bind(id)
        .execute(&state.repo.db)
        .await
        .map_err(|e| DomainError::Internal(e.into()))?
        .rows_affected();
    if n == 0 {
        return Err(DomainError::NotFound(id));
    }
    state.repo.audit(Some(auth.id), "medal.del", Some(id)).await;
    Ok(ok(serde_json::json!({ "deleted": id })))
}

#[derive(sqlx::FromRow, serde::Serialize)]
struct UserMedalRow {
    user_id: i64,
    username: String,
    medal_id: i64,
    medal_name: String,
    source: String,
    wearing: bool,
    granted_at: Option<chrono::DateTime<chrono::Utc>>,
}

#[derive(Deserialize)]
struct UserMedalQ {
    #[serde(default)]
    uid: Option<i64>,
    #[serde(default = "crate::admin_http::default_page")]
    page: i64,
    #[serde(default = "crate::admin_http::default_per_page")]
    per_page: i64,
}

#[get("/admin/user-medals")]
async fn admin_user_medals(
    req: HttpRequest,
    state: web::Data<std::sync::Arc<AppState>>,
    q: web::Query<UserMedalQ>,
) -> DomainResult<HttpResponse> {
    let _auth = staff(&req, &state).await?;
    if !(1..=100).contains(&q.per_page) {
        return Err(DomainError::Validation("per_page 取值 1-100".into()));
    }
    let rows: Vec<UserMedalRow> = sqlx::query_as(
        r#"SELECT um.user_id, u.username, um.medal_id, m.name AS medal_name,
                  um.source, um.wearing, um.granted_at
           FROM user_medals um
           JOIN users u ON u.id = um.user_id
           JOIN medals m ON m.id = um.medal_id
           WHERE ($1::bigint IS NULL OR um.user_id = $1)
           ORDER BY um.medal_id, um.user_id LIMIT $2 OFFSET $3"#,
    )
    .bind(q.uid)
    .bind(q.per_page)
    .bind((q.page.max(1) - 1) * q.per_page)
    .fetch_all(&state.repo.db)
    .await
    .map_err(|e| DomainError::Internal(e.into()))?;
    let total: i64 = sqlx::query_scalar(
        "SELECT count(*) FROM user_medals um WHERE ($1::bigint IS NULL OR um.user_id = $1)",
    )
    .bind(q.uid)
    .fetch_one(&state.repo.db)
    .await
    .unwrap_or(0);
    Ok(ok(
        serde_json::json!({ "rows": rows, "total": total, "page": q.page.max(1), "per_page": q.per_page }),
    ))
}

#[derive(Deserialize)]
struct UserMedalDel {
    user_id: i64,
    medal_id: i64,
}

/// 回收勋章（好学站 UserMedal 删除口径）
#[post("/admin/user-medals/delete")]
async fn admin_user_medal_revoke(
    req: HttpRequest,
    state: web::Data<std::sync::Arc<AppState>>,
    body: web::Json<UserMedalDel>,
) -> DomainResult<HttpResponse> {
    let auth = staff(&req, &state).await?;
    crate::authz::require_perm(&state, &auth, crate::authz::perm::MEDAL_MANAGE).await?;
    let n = sqlx::query("DELETE FROM user_medals WHERE user_id = $1 AND medal_id = $2")
        .bind(body.user_id)
        .bind(body.medal_id)
        .execute(&state.repo.db)
        .await
        .map_err(|e| DomainError::Internal(e.into()))?
        .rows_affected();
    if n == 0 {
        return Err(DomainError::NotFound(body.medal_id));
    }
    state
        .repo
        .audit(Some(auth.id), "medal.revoke", Some(body.user_id))
        .await;
    modify_log(
        &state.repo.db,
        body.user_id,
        Some(auth.id),
        &format!("回收勋章 #{}", body.medal_id),
    )
    .await;
    Ok(ok(serde_json::json!({ "ok": true })))
}

// ============ P2-8 道具 CRUD 与用户背包 ============

#[derive(sqlx::FromRow, serde::Serialize)]
struct ShopItemRow {
    id: i64,
    name: String,
    kind: String,
    price: i64,
    config: serde_json::Value,
    active: bool,
}

#[get("/admin/shop-items")]
async fn admin_shop_items(
    req: HttpRequest,
    state: web::Data<std::sync::Arc<AppState>>,
) -> DomainResult<HttpResponse> {
    let _auth = staff(&req, &state).await?;
    let rows: Vec<ShopItemRow> = sqlx::query_as(
        "SELECT id, name, kind, price, config, active FROM shop_items ORDER BY kind, id",
    )
    .fetch_all(&state.repo.db)
    .await
    .map_err(|e| DomainError::Internal(e.into()))?;
    Ok(ok(serde_json::to_value(rows).unwrap_or_default()))
}

#[derive(Deserialize)]
struct ShopItemReq {
    name: String,
    kind: String,
    #[serde(default)]
    price: Option<i64>,
    #[serde(default)]
    config: Option<serde_json::Value>,
    #[serde(default)]
    active: Option<bool>,
}

#[post("/admin/shop-items")]
async fn admin_shop_item_add(
    req: HttpRequest,
    state: web::Data<std::sync::Arc<AppState>>,
    body: web::Json<ShopItemReq>,
) -> DomainResult<HttpResponse> {
    let auth = staff(&req, &state).await?;
    crate::authz::require_perm(&state, &auth, crate::authz::perm::PROP_MANAGE).await?;
    if body.name.trim().is_empty() || body.kind.trim().is_empty() {
        return Err(DomainError::Validation("名称与类型必填".into()));
    }
    let id: i64 = sqlx::query_scalar(
        "INSERT INTO shop_items (name, kind, price, config, active) \
         VALUES ($1, $2, COALESCE($3, 0), COALESCE($4, '{}'::jsonb), COALESCE($5, TRUE)) RETURNING id",
    )
    .bind(body.name.trim())
    .bind(body.kind.trim())
    .bind(body.price)
    .bind(body.config.clone())
    .bind(body.active)
    .fetch_one(&state.repo.db)
    .await
    .map_err(|e| DomainError::Internal(e.into()))?;
    state.repo.audit(Some(auth.id), "prop.add", Some(id)).await;
    Ok(ok(serde_json::json!({ "id": id })))
}

#[put("/admin/shop-items/{id}")]
async fn admin_shop_item_update(
    req: HttpRequest,
    state: web::Data<std::sync::Arc<AppState>>,
    path: web::Path<i64>,
    body: web::Json<ShopItemReq>,
) -> DomainResult<HttpResponse> {
    let auth = staff(&req, &state).await?;
    crate::authz::require_perm(&state, &auth, crate::authz::perm::PROP_MANAGE).await?;
    let id = path.into_inner();
    let n = sqlx::query(
        "UPDATE shop_items SET name = $2, kind = $3, price = COALESCE($4, price), \
           config = COALESCE($5, config), active = COALESCE($6, active) WHERE id = $1",
    )
    .bind(id)
    .bind(body.name.trim())
    .bind(body.kind.trim())
    .bind(body.price)
    .bind(body.config.clone())
    .bind(body.active)
    .execute(&state.repo.db)
    .await
    .map_err(|e| DomainError::Internal(e.into()))?
    .rows_affected();
    if n == 0 {
        return Err(DomainError::NotFound(id));
    }
    state
        .repo
        .audit(Some(auth.id), "prop.update", Some(id))
        .await;
    Ok(ok(serde_json::json!({ "ok": true })))
}

#[delete("/admin/shop-items/{id}")]
async fn admin_shop_item_delete(
    req: HttpRequest,
    state: web::Data<std::sync::Arc<AppState>>,
    path: web::Path<i64>,
) -> DomainResult<HttpResponse> {
    let auth = staff(&req, &state).await?;
    crate::authz::require_perm(&state, &auth, crate::authz::perm::PROP_MANAGE).await?;
    let id = path.into_inner();
    let in_use: i64 = sqlx::query_scalar("SELECT count(*) FROM shop_orders WHERE item_id = $1")
        .bind(id)
        .fetch_one(&state.repo.db)
        .await
        .unwrap_or(0);
    if in_use > 0 {
        // 有持有/购买记录：只允许下架，不允许删（保留历史）
        let n = sqlx::query("UPDATE shop_items SET active = FALSE WHERE id = $1")
            .bind(id)
            .execute(&state.repo.db)
            .await
            .map_err(|e| DomainError::Internal(e.into()))?
            .rows_affected();
        state
            .repo
            .audit(Some(auth.id), "prop.disable", Some(id))
            .await;
        return Ok(ok(serde_json::json!({ "disabled": n > 0 })));
    }
    let n = sqlx::query("DELETE FROM shop_items WHERE id = $1")
        .bind(id)
        .execute(&state.repo.db)
        .await
        .map_err(|e| DomainError::Internal(e.into()))?
        .rows_affected();
    if n == 0 {
        return Err(DomainError::NotFound(id));
    }
    state.repo.audit(Some(auth.id), "prop.del", Some(id)).await;
    Ok(ok(serde_json::json!({ "deleted": id })))
}

#[derive(sqlx::FromRow, serde::Serialize)]
struct UserPropRow {
    order_id: i64,
    user_id: i64,
    username: String,
    item_id: i64,
    item_name: String,
    kind: String,
    price: i64,
    created_at: chrono::DateTime<chrono::Utc>,
}

#[derive(Deserialize)]
struct UserPropQ {
    #[serde(default)]
    uid: Option<i64>,
    #[serde(default = "crate::admin_http::default_page")]
    page: i64,
    #[serde(default = "crate::admin_http::default_per_page")]
    per_page: i64,
}

/// 用户背包 = shop_orders（购买与发放统一落单；即时生效类不产生持有）
#[get("/admin/user-props")]
async fn admin_user_props(
    req: HttpRequest,
    state: web::Data<std::sync::Arc<AppState>>,
    q: web::Query<UserPropQ>,
) -> DomainResult<HttpResponse> {
    let _auth = staff(&req, &state).await?;
    if !(1..=100).contains(&q.per_page) {
        return Err(DomainError::Validation("per_page 取值 1-100".into()));
    }
    let rows: Vec<UserPropRow> = sqlx::query_as(
        r#"SELECT o.id AS order_id, o.user_id, u.username, o.item_id, i.name AS item_name,
                  i.kind, o.price, o.created_at
           FROM shop_orders o
           JOIN users u ON u.id = o.user_id
           JOIN shop_items i ON i.id = o.item_id
           WHERE ($1::bigint IS NULL OR o.user_id = $1)
           ORDER BY o.id DESC LIMIT $2 OFFSET $3"#,
    )
    .bind(q.uid)
    .bind(q.per_page)
    .bind((q.page.max(1) - 1) * q.per_page)
    .fetch_all(&state.repo.db)
    .await
    .map_err(|e| DomainError::Internal(e.into()))?;
    let total: i64 = sqlx::query_scalar(
        "SELECT count(*) FROM shop_orders o WHERE ($1::bigint IS NULL OR o.user_id = $1)",
    )
    .bind(q.uid)
    .fetch_one(&state.repo.db)
    .await
    .unwrap_or(0);
    Ok(ok(
        serde_json::json!({ "rows": rows, "total": total, "page": q.page.max(1), "per_page": q.per_page }),
    ))
}

/// 背包回收：删除持有单（仅卡牌/装饰类入包道具；即时生效类不可撤）
#[delete("/admin/user-props/{order_id}")]
async fn admin_user_prop_revoke(
    req: HttpRequest,
    state: web::Data<std::sync::Arc<AppState>>,
    path: web::Path<i64>,
) -> DomainResult<HttpResponse> {
    let auth = staff(&req, &state).await?;
    crate::authz::require_perm(&state, &auth, crate::authz::perm::PROP_MANAGE).await?;
    let order_id = path.into_inner();
    let row: Option<(i64, String, String)> = sqlx::query_as(
        "SELECT o.user_id, i.kind, i.name FROM shop_orders o \
         JOIN shop_items i ON i.id = o.item_id WHERE o.id = $1",
    )
    .bind(order_id)
    .fetch_optional(&state.repo.db)
    .await
    .map_err(|e| DomainError::Internal(e.into()))?;
    let Some((uid, kind, name)) = row else {
        return Err(DomainError::NotFound(order_id));
    };
    const INSTANT_KINDS: [&str; 4] = ["upload_credit", "gift_spark", "invite", "temp_invite"];
    if INSTANT_KINDS.contains(&kind.as_str()) {
        return Err(DomainError::Validation(
            "即时生效类道具已入账，不可回收".into(),
        ));
    }
    sqlx::query("DELETE FROM shop_orders WHERE id = $1")
        .bind(order_id)
        .execute(&state.repo.db)
        .await
        .map_err(|e| DomainError::Internal(e.into()))?;
    state
        .repo
        .audit(Some(auth.id), "prop.revoke", Some(order_id))
        .await;
    modify_log(
        &state.repo.db,
        uid,
        Some(auth.id),
        &format!("回收道具「{name}」"),
    )
    .await;
    Ok(ok(serde_json::json!({ "ok": true })))
}

// ============ P2-9 用户批量操作 ============

#[derive(Deserialize)]
struct UserBatchReq {
    /// status（value 0/1/2）| class（value 等级数字）
    action: String,
    ids: Vec<i64>,
    value: i32,
    #[serde(default)]
    reason: Option<String>,
}

#[post("/admin/users/batch")]
async fn admin_users_batch(
    req: HttpRequest,
    state: web::Data<std::sync::Arc<AppState>>,
    body: web::Json<UserBatchReq>,
) -> DomainResult<HttpResponse> {
    let auth = staff(&req, &state).await?;
    let perm = match body.action.as_str() {
        "status" => crate::authz::perm::USER_STATUS,
        "class" => crate::authz::perm::USER_CLASS,
        _ => return Err(DomainError::Validation("未知批量动作".into())),
    };
    crate::authz::require_perm(&state, &auth, perm).await?;
    check_ids(&body.ids)?;
    if body.action == "status" && !(0..=2).contains(&body.value) {
        return Err(DomainError::Validation("status 取值 0/1/2".into()));
    }
    if body.action == "class" && !(1..=98).contains(&body.value) {
        return Err(DomainError::Validation("等级取值 1-98（站长除外）".into()));
    }
    let db = &state.repo.db;
    let mut updated: u64 = 0;
    let mut skipped: Vec<i64> = Vec::new();
    for uid in &body.ids {
        // 越权防护：只能操作严格低于自己等级的用户
        let target_class: Option<i32> =
            sqlx::query_scalar("SELECT class_id FROM users WHERE id = $1")
                .bind(uid)
                .fetch_optional(db)
                .await
                .map_err(|e| DomainError::Internal(e.into()))?;
        let Some(tc) = target_class else {
            skipped.push(*uid);
            continue;
        };
        if tc >= auth.class_id {
            skipped.push(*uid);
            continue;
        }
        let n = match body.action.as_str() {
            "status" => sqlx::query("UPDATE users SET status = $2 WHERE id = $1")
                .bind(uid)
                .bind(body.value as i16)
                .execute(db)
                .await
                .map_err(|e| DomainError::Internal(e.into()))?
                .rows_affected(),
            "class" => sqlx::query("UPDATE users SET class_id = $2 WHERE id = $1")
                .bind(uid)
                .bind(body.value)
                .execute(db)
                .await
                .map_err(|e| DomainError::Internal(e.into()))?
                .rows_affected(),
            _ => 0,
        };
        if n > 0 {
            updated += n;
            let content = match body.action.as_str() {
                "status" => format!(
                    "批量状态 → {}{}",
                    body.value,
                    body.reason
                        .as_deref()
                        .map(|r| format!("（{r}）"))
                        .unwrap_or_default()
                ),
                "class" => format!("批量等级 → {}", body.value),
                _ => String::new(),
            };
            modify_log(db, *uid, Some(auth.id), &content).await;
            state
                .repo
                .audit(
                    Some(auth.id),
                    &format!("user.batch.{}", body.action),
                    Some(*uid),
                )
                .await;
        }
    }
    Ok(ok(
        serde_json::json!({ "updated": updated as i64, "skipped": skipped }),
    ))
}

// ============ P2-10 按 IP 解封（登录记录页一键封/解封配套） ============

#[derive(Deserialize)]
struct BanByIpReq {
    ip: String,
}

#[post("/admin/bans/by-ip/delete")]
async fn admin_ban_delete_by_ip(
    req: HttpRequest,
    state: web::Data<std::sync::Arc<AppState>>,
    body: web::Json<BanByIpReq>,
) -> DomainResult<HttpResponse> {
    let auth = staff(&req, &state).await?;
    crate::authz::require_perm(&state, &auth, crate::authz::perm::BANS_MANAGE).await?;
    let ip = body.ip.trim();
    if ip.is_empty() {
        return Err(DomainError::Validation("IP 不能为空".into()));
    }
    let n = sqlx::query("DELETE FROM ip_bans WHERE ip = $1::inet")
        .bind(ip)
        .execute(&state.repo.db)
        .await
        .map_err(|e| DomainError::Internal(e.into()))?
        .rows_affected();
    state.repo.audit(Some(auth.id), "ban.del_by_ip", None).await;
    // 解封需立即作用于请求入口与 tracker（ip_bans 强制校验 + 防护缓存）
    crate::http::bump_guard_ver(&state).await;
    Ok(ok(serde_json::json!({ "deleted": n })))
}

// ============ P2-11 消息模板新增 / 删除 ============

#[derive(Deserialize)]
struct TemplateCreateReq {
    scene_key: String,
    subject: String,
    body: String,
    #[serde(default)]
    note: Option<String>,
}

#[post("/admin/message-templates")]
async fn admin_template_create(
    req: HttpRequest,
    state: web::Data<std::sync::Arc<AppState>>,
    body: web::Json<TemplateCreateReq>,
) -> DomainResult<HttpResponse> {
    let auth = staff(&req, &state).await?;
    let scene = body.scene_key.trim();
    if scene.is_empty() || body.subject.trim().is_empty() || body.body.trim().is_empty() {
        return Err(DomainError::Validation("场景键/主题/正文必填".into()));
    }
    if !scene
        .chars()
        .all(|c| c.is_ascii_lowercase() || c.is_ascii_digit() || c == '_')
    {
        return Err(DomainError::Validation(
            "场景键仅允许小写字母/数字/下划线".into(),
        ));
    }
    let exists: bool =
        sqlx::query_scalar("SELECT EXISTS(SELECT 1 FROM message_templates WHERE scene_key = $1)")
            .bind(scene)
            .fetch_one(&state.repo.db)
            .await
            .map_err(|e| DomainError::Internal(e.into()))?;
    if exists {
        return Err(DomainError::Validation("场景键已存在".into()));
    }
    let id: i64 = sqlx::query_scalar(
        "INSERT INTO message_templates (scene_key, subject, body, note) VALUES ($1, $2, $3, $4) RETURNING id",
    )
    .bind(scene)
    .bind(body.subject.trim())
    .bind(body.body.trim())
    .bind(body.note.clone())
    .fetch_one(&state.repo.db)
    .await
    .map_err(|e| DomainError::Internal(e.into()))?;
    state
        .repo
        .audit(Some(auth.id), "template.create", Some(id))
        .await;
    Ok(ok(serde_json::json!({ "id": id })))
}

#[delete("/admin/message-templates/{id}")]
async fn admin_template_delete(
    req: HttpRequest,
    state: web::Data<std::sync::Arc<AppState>>,
    path: web::Path<i64>,
) -> DomainResult<HttpResponse> {
    let auth = staff(&req, &state).await?;
    let id = path.into_inner();
    let n = sqlx::query("DELETE FROM message_templates WHERE id = $1")
        .bind(id)
        .execute(&state.repo.db)
        .await
        .map_err(|e| DomainError::Internal(e.into()))?
        .rows_affected();
    if n == 0 {
        return Err(DomainError::NotFound(id));
    }
    state
        .repo
        .audit(Some(auth.id), "template.delete", Some(id))
        .await;
    Ok(ok(serde_json::json!({ "deleted": id })))
}

// ============ P3-12 Section 多维体系 ============

#[derive(sqlx::FromRow, serde::Serialize)]
struct SectionModeRow {
    id: i32,
    name: String,
    show_source: bool,
    show_medium: bool,
    show_codec: bool,
    show_audio_codec: bool,
    show_standard: bool,
    show_processing: bool,
    show_team: bool,
    categories: i64,
}

#[get("/admin/section-modes")]
async fn section_modes_list(
    req: HttpRequest,
    state: web::Data<std::sync::Arc<AppState>>,
) -> DomainResult<HttpResponse> {
    let _auth = staff(&req, &state).await?;
    let rows: Vec<SectionModeRow> = sqlx::query_as(
        r#"SELECT m.id, m.name, m.show_source, m.show_medium, m.show_codec, m.show_audio_codec,
                  m.show_standard, m.show_processing, m.show_team,
                  (SELECT count(*) FROM categories c WHERE c.mode_id = m.id)::bigint AS categories
           FROM category_modes m ORDER BY m.id"#,
    )
    .fetch_all(&state.repo.db)
    .await
    .map_err(|e| DomainError::Internal(e.into()))?;
    Ok(ok(serde_json::to_value(rows).unwrap_or_default()))
}

#[derive(Deserialize)]
struct SectionModeReq {
    name: String,
    #[serde(default)]
    show_source: Option<bool>,
    #[serde(default)]
    show_medium: Option<bool>,
    #[serde(default)]
    show_codec: Option<bool>,
    #[serde(default)]
    show_audio_codec: Option<bool>,
    #[serde(default)]
    show_standard: Option<bool>,
    #[serde(default)]
    show_processing: Option<bool>,
    #[serde(default)]
    show_team: Option<bool>,
}

#[post("/admin/section-modes")]
async fn section_mode_add(
    req: HttpRequest,
    state: web::Data<std::sync::Arc<AppState>>,
    body: web::Json<SectionModeReq>,
) -> DomainResult<HttpResponse> {
    let auth = staff(&req, &state).await?;
    crate::authz::require_perm(&state, &auth, crate::authz::perm::CATEGORIES_MANAGE).await?;
    if body.name.trim().is_empty() {
        return Err(DomainError::Validation("模式名不能为空".into()));
    }
    let id: i32 = sqlx::query_scalar(
        "INSERT INTO category_modes (name, show_source, show_medium, show_codec, show_audio_codec, show_standard, show_processing, show_team) \
         VALUES ($1, COALESCE($2, TRUE), COALESCE($3, TRUE), COALESCE($4, TRUE), COALESCE($5, TRUE), COALESCE($6, TRUE), COALESCE($7, TRUE), COALESCE($8, TRUE)) RETURNING id",
    )
    .bind(body.name.trim())
    .bind(body.show_source)
    .bind(body.show_medium)
    .bind(body.show_codec)
    .bind(body.show_audio_codec)
    .bind(body.show_standard)
    .bind(body.show_processing)
    .bind(body.show_team)
    .fetch_one(&state.repo.db)
    .await
    .map_err(|e| DomainError::Internal(e.into()))?;
    state
        .repo
        .audit(Some(auth.id), "section_mode.add", Some(id as i64))
        .await;
    Ok(ok(serde_json::json!({ "id": id })))
}

#[put("/admin/section-modes/{id}")]
async fn section_mode_update(
    req: HttpRequest,
    state: web::Data<std::sync::Arc<AppState>>,
    path: web::Path<i32>,
    body: web::Json<SectionModeReq>,
) -> DomainResult<HttpResponse> {
    let auth = staff(&req, &state).await?;
    crate::authz::require_perm(&state, &auth, crate::authz::perm::CATEGORIES_MANAGE).await?;
    let id = path.into_inner();
    let n = sqlx::query(
        "UPDATE category_modes SET name = $2, show_source = COALESCE($3, show_source), \
           show_medium = COALESCE($4, show_medium), show_codec = COALESCE($5, show_codec), \
           show_audio_codec = COALESCE($6, show_audio_codec), show_standard = COALESCE($7, show_standard), \
           show_processing = COALESCE($8, show_processing), show_team = COALESCE($9, show_team) WHERE id = $1",
    )
    .bind(id)
    .bind(body.name.trim())
    .bind(body.show_source)
    .bind(body.show_medium)
    .bind(body.show_codec)
    .bind(body.show_audio_codec)
    .bind(body.show_standard)
    .bind(body.show_processing)
    .bind(body.show_team)
    .execute(&state.repo.db)
    .await
    .map_err(|e| DomainError::Internal(e.into()))?
    .rows_affected();
    if n == 0 {
        return Err(DomainError::NotFound(id as i64));
    }
    state
        .repo
        .audit(Some(auth.id), "section_mode.update", Some(id as i64))
        .await;
    Ok(ok(serde_json::json!({ "ok": true })))
}

#[delete("/admin/section-modes/{id}")]
async fn section_mode_delete(
    req: HttpRequest,
    state: web::Data<std::sync::Arc<AppState>>,
    path: web::Path<i32>,
) -> DomainResult<HttpResponse> {
    let auth = staff(&req, &state).await?;
    crate::authz::require_perm(&state, &auth, crate::authz::perm::CATEGORIES_MANAGE).await?;
    let id = path.into_inner();
    if id == 1 {
        return Err(DomainError::Validation("默认模式不可删除".into()));
    }
    let used: i64 = sqlx::query_scalar("SELECT count(*) FROM categories WHERE mode_id = $1")
        .bind(id)
        .fetch_one(&state.repo.db)
        .await
        .unwrap_or(0);
    if used > 0 {
        sqlx::query("UPDATE categories SET mode_id = 1 WHERE mode_id = $1")
            .bind(id)
            .execute(&state.repo.db)
            .await
            .map_err(|e| DomainError::Internal(e.into()))?;
    }
    let n = sqlx::query("DELETE FROM category_modes WHERE id = $1")
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
        .audit(Some(auth.id), "section_mode.del", Some(id as i64))
        .await;
    Ok(ok(serde_json::json!({ "deleted": id })))
}

pub(crate) const SECTION_KINDS: [&str; 6] = [
    "codec",
    "audio_codec",
    "standard",
    "team",
    "source",
    "processing",
];
/// media/grades/editions 复用 0001 既有三表
const LEGACY_KINDS: [&str; 3] = ["media", "grades", "editions"];

#[derive(sqlx::FromRow, serde::Serialize)]
struct SectionDictRow {
    id: i64,
    kind: String,
    name: String,
    sort: i32,
    mode_id: Option<i32>,
}

#[derive(Deserialize)]
struct SectionDictQ {
    #[serde(default)]
    kind: Option<String>,
}

/// 维度字典统一读取（管理端全量；发布表单用 /section-dict 公开读）
#[get("/admin/section-dict")]
async fn section_dict_admin(
    req: HttpRequest,
    state: web::Data<std::sync::Arc<AppState>>,
    q: web::Query<SectionDictQ>,
) -> DomainResult<HttpResponse> {
    let _auth = staff(&req, &state).await?;
    let rows: Vec<SectionDictRow> = section_dict_rows(&state.repo.db, q.kind.as_deref()).await?;
    Ok(ok(serde_json::to_value(rows).unwrap_or_default()))
}

async fn section_dict_rows(
    db: &sqlx::PgPool,
    kind: Option<&str>,
) -> DomainResult<Vec<SectionDictRow>> {
    let k = kind.unwrap_or("");
    if LEGACY_KINDS.contains(&k) {
        let table = k; // 表名来自常量白名单，非用户输入
        let sql = format!("SELECT id::bigint AS id, '{k}' AS kind, name, sort, NULL::int AS mode_id FROM {table} ORDER BY sort, id");
        return sqlx::query_as(&sql)
            .fetch_all(db)
            .await
            .map_err(|e| DomainError::Internal(e.into()));
    }
    sqlx::query_as(
        "SELECT id, kind, name, sort, mode_id FROM section_dict \
         WHERE ($1::text IS NULL OR kind = $1) ORDER BY kind, sort, id",
    )
    .bind(kind.filter(|k| SECTION_KINDS.contains(k)))
    .fetch_all(db)
    .await
    .map_err(|e| DomainError::Internal(e.into()))
}

#[derive(Deserialize)]
struct SectionDictReq {
    kind: String,
    name: String,
    #[serde(default)]
    sort: Option<i32>,
    #[serde(default)]
    mode_id: Option<i32>,
}

#[post("/admin/section-dict")]
async fn section_dict_add(
    req: HttpRequest,
    state: web::Data<std::sync::Arc<AppState>>,
    body: web::Json<SectionDictReq>,
) -> DomainResult<HttpResponse> {
    let auth = staff(&req, &state).await?;
    crate::authz::require_perm(&state, &auth, crate::authz::perm::CATEGORIES_MANAGE).await?;
    let kind = body.kind.as_str();
    if body.name.trim().is_empty() {
        return Err(DomainError::Validation("名称不能为空".into()));
    }
    if LEGACY_KINDS.contains(&kind) {
        let id: i32 = sqlx::query_scalar(&format!(
            "INSERT INTO {kind} (name, sort) VALUES ($1, COALESCE($2, 0)) RETURNING id"
        ))
        .bind(body.name.trim())
        .bind(body.sort)
        .fetch_one(&state.repo.db)
        .await
        .map_err(|e| DomainError::Internal(e.into()))?;
        state
            .repo
            .audit(Some(auth.id), "section_dict.add", Some(id as i64))
            .await;
        return Ok(ok(serde_json::json!({ "id": id })));
    }
    if !SECTION_KINDS.contains(&kind) {
        return Err(DomainError::Validation("未知维度".into()));
    }
    let id: i64 = sqlx::query_scalar(
        "INSERT INTO section_dict (kind, name, sort, mode_id) VALUES ($1, $2, COALESCE($3, 0), $4) RETURNING id",
    )
    .bind(kind)
    .bind(body.name.trim())
    .bind(body.sort)
    .bind(body.mode_id)
    .fetch_one(&state.repo.db)
    .await
    .map_err(|e| DomainError::Internal(e.into()))?;
    state
        .repo
        .audit(Some(auth.id), "section_dict.add", Some(id))
        .await;
    Ok(ok(serde_json::json!({ "id": id })))
}

#[put("/admin/section-dict/{id}")]
async fn section_dict_update(
    req: HttpRequest,
    state: web::Data<std::sync::Arc<AppState>>,
    path: web::Path<i64>,
    body: web::Json<SectionDictReq>,
) -> DomainResult<HttpResponse> {
    let auth = staff(&req, &state).await?;
    crate::authz::require_perm(&state, &auth, crate::authz::perm::CATEGORIES_MANAGE).await?;
    let id = path.into_inner();
    let kind = body.kind.as_str();
    let n = if LEGACY_KINDS.contains(&kind) {
        sqlx::query(&format!(
            "UPDATE {kind} SET name = $2, sort = COALESCE($3, sort) WHERE id = $1"
        ))
        .bind(id as i32)
        .bind(body.name.trim())
        .bind(body.sort)
        .execute(&state.repo.db)
        .await
        .map_err(|e| DomainError::Internal(e.into()))?
        .rows_affected()
    } else if SECTION_KINDS.contains(&kind) {
        sqlx::query(
            "UPDATE section_dict SET name = $2, sort = COALESCE($3, sort), mode_id = $4 WHERE id = $1",
        )
        .bind(id)
        .bind(body.name.trim())
        .bind(body.sort)
        .bind(body.mode_id)
        .execute(&state.repo.db)
        .await
        .map_err(|e| DomainError::Internal(e.into()))?
        .rows_affected()
    } else {
        return Err(DomainError::Validation("未知维度".into()));
    };
    if n == 0 {
        return Err(DomainError::NotFound(id));
    }
    state
        .repo
        .audit(Some(auth.id), "section_dict.update", Some(id))
        .await;
    Ok(ok(serde_json::json!({ "ok": true })))
}

#[delete("/admin/section-dict/{id}")]
async fn section_dict_delete(
    req: HttpRequest,
    state: web::Data<std::sync::Arc<AppState>>,
    path: web::Path<i64>,
    q: web::Query<SectionDictQ>,
) -> DomainResult<HttpResponse> {
    let auth = staff(&req, &state).await?;
    crate::authz::require_perm(&state, &auth, crate::authz::perm::CATEGORIES_MANAGE).await?;
    let id = path.into_inner();
    let kind = q.kind.clone().unwrap_or_default();
    let n = if LEGACY_KINDS.contains(&kind.as_str()) {
        sqlx::query(&format!("DELETE FROM {kind} WHERE id = $1"))
            .bind(id as i32)
            .execute(&state.repo.db)
            .await
            .map_err(|e| DomainError::Internal(e.into()))?
            .rows_affected()
    } else if SECTION_KINDS.contains(&kind.as_str()) {
        sqlx::query("DELETE FROM section_dict WHERE id = $1")
            .bind(id)
            .execute(&state.repo.db)
            .await
            .map_err(|e| DomainError::Internal(e.into()))?
            .rows_affected()
    } else {
        return Err(DomainError::Validation("未知维度".into()));
    };
    if n == 0 {
        return Err(DomainError::NotFound(id));
    }
    state
        .repo
        .audit(Some(auth.id), "section_dict.del", Some(id))
        .await;
    Ok(ok(serde_json::json!({ "deleted": id })))
}

/// 发布表单/筛选公开读（匿名可读：仅字典名称，与 site-profile 同级）
#[get("/section-dict")]
async fn section_dict_public(
    _req: HttpRequest,
    state: web::Data<std::sync::Arc<AppState>>,
) -> DomainResult<HttpResponse> {
    let mut out = serde_json::Map::new();
    for kind in LEGACY_KINDS {
        let rows = section_dict_rows(&state.repo.db, Some(kind)).await?;
        out.insert(
            kind.to_string(),
            serde_json::to_value(rows).unwrap_or_default(),
        );
    }
    for kind in SECTION_KINDS {
        let rows = section_dict_rows(&state.repo.db, Some(kind)).await?;
        out.insert(
            kind.to_string(),
            serde_json::to_value(rows).unwrap_or_default(),
        );
    }
    let modes: Vec<SectionModeRow> = sqlx::query_as(
        r#"SELECT m.id, m.name, m.show_source, m.show_medium, m.show_codec, m.show_audio_codec,
                  m.show_standard, m.show_processing, m.show_team,
                  (SELECT count(*) FROM categories c WHERE c.mode_id = m.id)::bigint AS categories
           FROM category_modes m ORDER BY m.id"#,
    )
    .fetch_all(&state.repo.db)
    .await
    .map_err(|e| DomainError::Internal(e.into()))?;
    out.insert(
        "modes".into(),
        serde_json::to_value(modes).unwrap_or_default(),
    );
    Ok(ok(serde_json::Value::Object(out)))
}

/// 分类归属模式 + 自动过审开关
#[derive(Deserialize)]
struct CategoryFlagsReq {
    #[serde(default)]
    mode_id: Option<i32>,
    #[serde(default)]
    auto_approve: Option<bool>,
}

#[put("/admin/categories/{id}/flags")]
async fn category_flags(
    req: HttpRequest,
    state: web::Data<std::sync::Arc<AppState>>,
    path: web::Path<i32>,
    body: web::Json<CategoryFlagsReq>,
) -> DomainResult<HttpResponse> {
    let auth = staff(&req, &state).await?;
    crate::authz::require_perm(&state, &auth, crate::authz::perm::CATEGORIES_MANAGE).await?;
    let id = path.into_inner();
    let n = sqlx::query(
        "UPDATE categories SET mode_id = $2, auto_approve = COALESCE($3, auto_approve) WHERE id = $1",
    )
    .bind(id)
    .bind(body.mode_id)
    .bind(body.auto_approve)
    .execute(&state.repo.db)
    .await
    .map_err(|e| DomainError::Internal(e.into()))?
    .rows_affected();
    if n == 0 {
        return Err(DomainError::NotFound(id as i64));
    }
    state
        .repo
        .audit(Some(auth.id), "category.flags", Some(id as i64))
        .await;
    Ok(ok(serde_json::json!({ "ok": true })))
}

// ============ P3-13 考核岗位类型 CRUD ============

#[derive(sqlx::FromRow, serde::Serialize)]
struct JixiaoTypeRow {
    id: i64,
    name: String,
    metrics: serde_json::Value,
    base_pay: i64,
    min_requirements: serde_json::Value,
    bonus_rules: serde_json::Value,
}

#[get("/admin/jixiao-types")]
async fn jixiao_types_list(
    req: HttpRequest,
    state: web::Data<std::sync::Arc<AppState>>,
) -> DomainResult<HttpResponse> {
    let _auth = staff(&req, &state).await?;
    let rows: Vec<JixiaoTypeRow> = sqlx::query_as(
        "SELECT id, name, metrics, base_pay, min_requirements, bonus_rules FROM jixiao_types ORDER BY id",
    )
    .fetch_all(&state.repo.db)
    .await
    .map_err(|e| DomainError::Internal(e.into()))?;
    Ok(ok(serde_json::to_value(rows).unwrap_or_default()))
}

#[derive(Deserialize)]
struct JixiaoTypeReq {
    name: String,
    #[serde(default)]
    metrics: Option<serde_json::Value>,
    #[serde(default)]
    base_pay: Option<i64>,
    #[serde(default)]
    min_requirements: Option<serde_json::Value>,
    #[serde(default)]
    bonus_rules: Option<serde_json::Value>,
}

#[post("/admin/jixiao-types")]
async fn jixiao_type_add(
    req: HttpRequest,
    state: web::Data<std::sync::Arc<AppState>>,
    body: web::Json<JixiaoTypeReq>,
) -> DomainResult<HttpResponse> {
    let auth = staff(&req, &state).await?;
    crate::authz::require_perm(&state, &auth, crate::authz::perm::EXAM_MANAGE).await?;
    if body.name.trim().is_empty() {
        return Err(DomainError::Validation("岗位名不能为空".into()));
    }
    let id: i64 = sqlx::query_scalar(
        "INSERT INTO jixiao_types (name, metrics, base_pay, min_requirements, bonus_rules) \
         VALUES ($1, COALESCE($2, '{}'::jsonb), COALESCE($3, 0), COALESCE($4, '{}'::jsonb), COALESCE($5, '{}'::jsonb)) RETURNING id",
    )
    .bind(body.name.trim())
    .bind(body.metrics.clone())
    .bind(body.base_pay)
    .bind(body.min_requirements.clone())
    .bind(body.bonus_rules.clone())
    .fetch_one(&state.repo.db)
    .await
    .map_err(|e| DomainError::Internal(e.into()))?;
    state
        .repo
        .audit(Some(auth.id), "jixiao_type.add", Some(id))
        .await;
    Ok(ok(serde_json::json!({ "id": id })))
}

#[put("/admin/jixiao-types/{id}")]
async fn jixiao_type_update(
    req: HttpRequest,
    state: web::Data<std::sync::Arc<AppState>>,
    path: web::Path<i64>,
    body: web::Json<JixiaoTypeReq>,
) -> DomainResult<HttpResponse> {
    let auth = staff(&req, &state).await?;
    crate::authz::require_perm(&state, &auth, crate::authz::perm::EXAM_MANAGE).await?;
    let id = path.into_inner();
    let n = sqlx::query(
        "UPDATE jixiao_types SET name = $2, metrics = COALESCE($3, metrics), base_pay = COALESCE($4, base_pay), \
           min_requirements = COALESCE($5, min_requirements), bonus_rules = COALESCE($6, bonus_rules) WHERE id = $1",
    )
    .bind(id)
    .bind(body.name.trim())
    .bind(body.metrics.clone())
    .bind(body.base_pay)
    .bind(body.min_requirements.clone())
    .bind(body.bonus_rules.clone())
    .execute(&state.repo.db)
    .await
    .map_err(|e| DomainError::Internal(e.into()))?
    .rows_affected();
    if n == 0 {
        return Err(DomainError::NotFound(id));
    }
    state
        .repo
        .audit(Some(auth.id), "jixiao_type.update", Some(id))
        .await;
    Ok(ok(serde_json::json!({ "ok": true })))
}

#[delete("/admin/jixiao-types/{id}")]
async fn jixiao_type_delete(
    req: HttpRequest,
    state: web::Data<std::sync::Arc<AppState>>,
    path: web::Path<i64>,
) -> DomainResult<HttpResponse> {
    let auth = staff(&req, &state).await?;
    crate::authz::require_perm(&state, &auth, crate::authz::perm::EXAM_MANAGE).await?;
    let id = path.into_inner();
    let used: i64 = sqlx::query_scalar("SELECT count(*) FROM jixiao_claims WHERE type_id = $1")
        .bind(id)
        .fetch_one(&state.repo.db)
        .await
        .unwrap_or(0);
    if used > 0 {
        return Err(DomainError::Validation(
            "已有考核登记引用该岗位，不可删除".into(),
        ));
    }
    let n = sqlx::query("DELETE FROM jixiao_types WHERE id = $1")
        .bind(id)
        .execute(&state.repo.db)
        .await
        .map_err(|e| DomainError::Internal(e.into()))?
        .rows_affected();
    if n == 0 {
        return Err(DomainError::NotFound(id));
    }
    state
        .repo
        .audit(Some(auth.id), "jixiao_type.del", Some(id))
        .await;
    Ok(ok(serde_json::json!({ "deleted": id })))
}

// ============ P3-14 任务定义 CRUD ============

#[derive(sqlx::FromRow, serde::Serialize)]
struct TaskRow {
    id: i64,
    name: String,
    metric: serde_json::Value,
    starts_at: chrono::DateTime<chrono::Utc>,
    ends_at: chrono::DateTime<chrono::Utc>,
    target_class: i32,
    reward: i64,
    penalty: i64,
    claim_limit: Option<i32>,
}

#[get("/admin/tasks")]
async fn tasks_list(
    req: HttpRequest,
    state: web::Data<std::sync::Arc<AppState>>,
) -> DomainResult<HttpResponse> {
    let _auth = staff(&req, &state).await?;
    let rows: Vec<TaskRow> = sqlx::query_as(
        "SELECT id, name, metric, starts_at, ends_at, target_class, reward, penalty, claim_limit \
         FROM tasks ORDER BY id DESC",
    )
    .fetch_all(&state.repo.db)
    .await
    .map_err(|e| DomainError::Internal(e.into()))?;
    Ok(ok(serde_json::to_value(rows).unwrap_or_default()))
}

#[derive(Deserialize)]
struct TaskReq {
    name: String,
    #[serde(default)]
    metric: Option<serde_json::Value>,
    starts_at: chrono::DateTime<chrono::Utc>,
    ends_at: chrono::DateTime<chrono::Utc>,
    #[serde(default)]
    target_class: Option<i32>,
    #[serde(default)]
    reward: Option<i64>,
    #[serde(default)]
    penalty: Option<i64>,
    #[serde(default)]
    claim_limit: Option<i32>,
}

#[post("/admin/tasks")]
async fn task_add(
    req: HttpRequest,
    state: web::Data<std::sync::Arc<AppState>>,
    body: web::Json<TaskReq>,
) -> DomainResult<HttpResponse> {
    let auth = staff(&req, &state).await?;
    crate::authz::require_perm(&state, &auth, crate::authz::perm::TASK_MANAGE).await?;
    if body.name.trim().is_empty() || body.ends_at <= body.starts_at {
        return Err(DomainError::Validation(
            "任务名必填且结束时间需晚于开始".into(),
        ));
    }
    let id: i64 = sqlx::query_scalar(
        "INSERT INTO tasks (name, metric, starts_at, ends_at, target_class, reward, penalty, claim_limit) \
         VALUES ($1, COALESCE($2, '{}'::jsonb), $3, $4, COALESCE($5, 0), COALESCE($6, 0), COALESCE($7, 0), $8) RETURNING id",
    )
    .bind(body.name.trim())
    .bind(body.metric.clone())
    .bind(body.starts_at)
    .bind(body.ends_at)
    .bind(body.target_class)
    .bind(body.reward)
    .bind(body.penalty)
    .bind(body.claim_limit)
    .fetch_one(&state.repo.db)
    .await
    .map_err(|e| DomainError::Internal(e.into()))?;
    state.repo.audit(Some(auth.id), "task.add", Some(id)).await;
    Ok(ok(serde_json::json!({ "id": id })))
}

#[put("/admin/tasks/{id}")]
async fn task_update(
    req: HttpRequest,
    state: web::Data<std::sync::Arc<AppState>>,
    path: web::Path<i64>,
    body: web::Json<TaskReq>,
) -> DomainResult<HttpResponse> {
    let auth = staff(&req, &state).await?;
    crate::authz::require_perm(&state, &auth, crate::authz::perm::TASK_MANAGE).await?;
    let id = path.into_inner();
    if body.ends_at <= body.starts_at {
        return Err(DomainError::Validation("结束时间需晚于开始".into()));
    }
    let n = sqlx::query(
        "UPDATE tasks SET name = $2, metric = COALESCE($3, metric), starts_at = $4, ends_at = $5, \
           target_class = COALESCE($6, target_class), reward = COALESCE($7, reward), \
           penalty = COALESCE($8, penalty), claim_limit = $9 WHERE id = $1",
    )
    .bind(id)
    .bind(body.name.trim())
    .bind(body.metric.clone())
    .bind(body.starts_at)
    .bind(body.ends_at)
    .bind(body.target_class)
    .bind(body.reward)
    .bind(body.penalty)
    .bind(body.claim_limit)
    .execute(&state.repo.db)
    .await
    .map_err(|e| DomainError::Internal(e.into()))?
    .rows_affected();
    if n == 0 {
        return Err(DomainError::NotFound(id));
    }
    state
        .repo
        .audit(Some(auth.id), "task.update", Some(id))
        .await;
    Ok(ok(serde_json::json!({ "ok": true })))
}

#[delete("/admin/tasks/{id}")]
async fn task_delete(
    req: HttpRequest,
    state: web::Data<std::sync::Arc<AppState>>,
    path: web::Path<i64>,
) -> DomainResult<HttpResponse> {
    let auth = staff(&req, &state).await?;
    crate::authz::require_perm(&state, &auth, crate::authz::perm::TASK_MANAGE).await?;
    let id = path.into_inner();
    let n = sqlx::query("DELETE FROM tasks WHERE id = $1")
        .bind(id)
        .execute(&state.repo.db)
        .await
        .map_err(|e| DomainError::Internal(e.into()))?
        .rows_affected();
    if n == 0 {
        return Err(DomainError::NotFound(id));
    }
    state.repo.audit(Some(auth.id), "task.del", Some(id)).await;
    Ok(ok(serde_json::json!({ "deleted": id })))
}

// ============ P3-16 Tracker URL 管理 ============

#[derive(sqlx::FromRow, serde::Serialize)]
struct TrackerUrlRow {
    id: i32,
    url: String,
    is_default: bool,
    enabled: bool,
    priority: i32,
    updated_at: chrono::DateTime<chrono::Utc>,
}

#[get("/admin/tracker-urls")]
async fn tracker_urls_list(
    req: HttpRequest,
    state: web::Data<std::sync::Arc<AppState>>,
) -> DomainResult<HttpResponse> {
    let _auth = staff(&req, &state).await?;
    let rows: Vec<TrackerUrlRow> = sqlx::query_as(
        "SELECT id, url, is_default, enabled, priority, updated_at FROM tracker_urls ORDER BY priority, id",
    )
    .fetch_all(&state.repo.db)
    .await
    .map_err(|e| DomainError::Internal(e.into()))?;
    Ok(ok(serde_json::to_value(rows).unwrap_or_default()))
}

#[derive(Deserialize)]
struct TrackerUrlReq {
    url: String,
    #[serde(default)]
    is_default: Option<bool>,
    #[serde(default)]
    enabled: Option<bool>,
    #[serde(default)]
    priority: Option<i32>,
}

#[post("/admin/tracker-urls")]
async fn tracker_url_add(
    req: HttpRequest,
    state: web::Data<std::sync::Arc<AppState>>,
    body: web::Json<TrackerUrlReq>,
) -> DomainResult<HttpResponse> {
    let auth = staff(&req, &state).await?;
    crate::authz::require_perm(&state, &auth, crate::authz::perm::TRACKER_MANAGE).await?;
    let url = body.url.trim();
    if !url.starts_with("https://") && !url.starts_with("http://") {
        return Err(DomainError::Validation("URL 需以 http(s):// 开头".into()));
    }
    // 同 URL 不允许重复登记（announce 列表会重复下发同一地址）
    let dup: bool = sqlx::query_scalar("SELECT EXISTS(SELECT 1 FROM tracker_urls WHERE url = $1)")
        .bind(url)
        .fetch_one(&state.repo.db)
        .await
        .unwrap_or(false);
    if dup {
        return Err(DomainError::Validation("该 Tracker URL 已存在".into()));
    }
    let id: i32 = sqlx::query_scalar(
        "INSERT INTO tracker_urls (url, is_default, enabled, priority) VALUES ($1, COALESCE($2, FALSE), COALESCE($3, TRUE), COALESCE($4, 0)) RETURNING id",
    )
    .bind(url)
    .bind(body.is_default)
    .bind(body.enabled)
    .bind(body.priority)
    .fetch_one(&state.repo.db)
    .await
    .map_err(|e| DomainError::Internal(e.into()))?;
    if body.is_default.unwrap_or(false) {
        sqlx::query("UPDATE tracker_urls SET is_default = FALSE WHERE id <> $1")
            .bind(id)
            .execute(&state.repo.db)
            .await
            .map_err(|e| DomainError::Internal(e.into()))?;
    }
    state
        .repo
        .audit(Some(auth.id), "tracker_url.add", Some(id as i64))
        .await;
    Ok(ok(serde_json::json!({ "id": id })))
}

#[put("/admin/tracker-urls/{id}")]
async fn tracker_url_update(
    req: HttpRequest,
    state: web::Data<std::sync::Arc<AppState>>,
    path: web::Path<i32>,
    body: web::Json<TrackerUrlReq>,
) -> DomainResult<HttpResponse> {
    let auth = staff(&req, &state).await?;
    crate::authz::require_perm(&state, &auth, crate::authz::perm::TRACKER_MANAGE).await?;
    let id = path.into_inner();
    let n = sqlx::query(
        "UPDATE tracker_urls SET url = $2, is_default = COALESCE($3, is_default), \
           enabled = COALESCE($4, enabled), priority = COALESCE($5, priority), updated_at = now() WHERE id = $1",
    )
    .bind(id)
    .bind(body.url.trim())
    .bind(body.is_default)
    .bind(body.enabled)
    .bind(body.priority)
    .execute(&state.repo.db)
    .await
    .map_err(|e| DomainError::Internal(e.into()))?
    .rows_affected();
    if n == 0 {
        return Err(DomainError::NotFound(id as i64));
    }
    if body.is_default.unwrap_or(false) {
        sqlx::query("UPDATE tracker_urls SET is_default = FALSE WHERE id <> $1")
            .bind(id)
            .execute(&state.repo.db)
            .await
            .map_err(|e| DomainError::Internal(e.into()))?;
    } else if body.is_default == Some(false) {
        // 显式取消默认：不允许，否则站点可能没有默认 announce 地址
        return Err(DomainError::Validation(
            "不能直接取消默认地址，请把其他地址设为默认".into(),
        ));
    }
    state
        .repo
        .audit(Some(auth.id), "tracker_url.update", Some(id as i64))
        .await;
    Ok(ok(serde_json::json!({ "ok": true })))
}

#[delete("/admin/tracker-urls/{id}")]
async fn tracker_url_delete(
    req: HttpRequest,
    state: web::Data<std::sync::Arc<AppState>>,
    path: web::Path<i32>,
) -> DomainResult<HttpResponse> {
    let auth = staff(&req, &state).await?;
    crate::authz::require_perm(&state, &auth, crate::authz::perm::TRACKER_MANAGE).await?;
    let id = path.into_inner();
    // 删除后至少保留一条启用地址；默认地址不可直接删（先转移默认位再删）
    let row: Option<(bool, bool)> =
        sqlx::query_as("SELECT is_default, enabled FROM tracker_urls WHERE id = $1")
            .bind(id)
            .fetch_optional(&state.repo.db)
            .await
            .map_err(|e| DomainError::Internal(e.into()))?;
    let Some((is_default, _enabled)) = row else {
        return Err(DomainError::NotFound(id as i64));
    };
    if is_default {
        return Err(DomainError::Validation(
            "默认地址不可删除，请先把其他地址设为默认".into(),
        ));
    }
    let left: i64 =
        sqlx::query_scalar("SELECT count(*) FROM tracker_urls WHERE enabled AND id <> $1")
            .bind(id)
            .fetch_one(&state.repo.db)
            .await
            .unwrap_or(0);
    if left == 0 {
        return Err(DomainError::Validation(
            "至少保留一条启用的 Tracker 地址".into(),
        ));
    }
    let n = sqlx::query("DELETE FROM tracker_urls WHERE id = $1")
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
        .audit(Some(auth.id), "tracker_url.del", Some(id as i64))
        .await;
    Ok(ok(serde_json::json!({ "deleted": id })))
}

// ============ P2-6b 管理员改名（写入 username_change_logs） ============

#[derive(Deserialize)]
struct RenameReq {
    new_name: String,
}

#[post("/admin/users/{id}/rename")]
async fn admin_user_rename(
    req: HttpRequest,
    state: web::Data<std::sync::Arc<AppState>>,
    path: web::Path<i64>,
    body: web::Json<RenameReq>,
) -> DomainResult<HttpResponse> {
    let auth = staff(&req, &state).await?;
    crate::authz::require_perm(&state, &auth, crate::authz::perm::USER_RESETPASS).await?;
    let uid = path.into_inner();
    let new_name = body.new_name.trim();
    if new_name.is_empty() || new_name.len() > 32 {
        return Err(DomainError::Validation("用户名长度 1-32".into()));
    }
    let db = &state.repo.db;
    let old: Option<String> = sqlx::query_scalar("SELECT username FROM users WHERE id = $1")
        .bind(uid)
        .fetch_optional(db)
        .await
        .map_err(|e| DomainError::Internal(e.into()))?;
    let Some(old_name) = old else {
        return Err(DomainError::NotFound(uid));
    };
    if old_name == new_name {
        return Ok(ok(serde_json::json!({ "ok": true })));
    }
    let taken: bool =
        sqlx::query_scalar("SELECT EXISTS(SELECT 1 FROM users WHERE username = $1 AND id <> $2)")
            .bind(new_name)
            .bind(uid)
            .fetch_one(db)
            .await
            .map_err(|e| DomainError::Internal(e.into()))?;
    if taken {
        return Err(DomainError::Validation("用户名已被占用".into()));
    }
    sqlx::query("UPDATE users SET username = $2 WHERE id = $1")
        .bind(uid)
        .bind(new_name)
        .execute(db)
        .await
        .map_err(|e| DomainError::Internal(e.into()))?;
    sqlx::query(
        "INSERT INTO username_change_logs (uid, old_name, new_name, operator) VALUES ($1, $2, $3, $4)",
    )
    .bind(uid)
    .bind(&old_name)
    .bind(new_name)
    .bind(auth.id)
    .execute(db)
    .await
    .map_err(|e| DomainError::Internal(e.into()))?;
    state
        .repo
        .audit(Some(auth.id), "user.rename", Some(uid))
        .await;
    modify_log(
        db,
        uid,
        Some(auth.id),
        &format!("改名 {old_name} → {new_name}"),
    )
    .await;
    Ok(ok(serde_json::json!({ "ok": true })))
}

// ============ 职务字典 CRUD（0064：roles.manage 仅站长） ============

#[derive(sqlx::FromRow, serde::Serialize)]
struct RoleDefRow {
    key: String,
    name: String,
    descr: Option<String>,
    sort: i32,
}

#[get("/admin/roles-dict")]
async fn roles_dict_list(
    req: HttpRequest,
    state: web::Data<std::sync::Arc<AppState>>,
) -> DomainResult<HttpResponse> {
    let _auth = staff(&req, &state).await?;
    let rows: Vec<RoleDefRow> =
        sqlx::query_as("SELECT key, name, descr, sort FROM roles ORDER BY sort, key")
            .fetch_all(&state.repo.db)
            .await
            .map_err(|e| DomainError::Internal(e.into()))?;
    Ok(ok(serde_json::to_value(rows).unwrap_or_default()))
}

#[derive(Deserialize)]
struct RoleDefReq {
    key: String,
    name: String,
    #[serde(default)]
    descr: Option<String>,
    #[serde(default)]
    sort: Option<i32>,
}

#[post("/admin/roles")]
async fn roles_dict_add(
    req: HttpRequest,
    state: web::Data<std::sync::Arc<AppState>>,
    body: web::Json<RoleDefReq>,
) -> DomainResult<HttpResponse> {
    let auth = staff(&req, &state).await?;
    crate::authz::require_perm(&state, &auth, crate::authz::perm::ROLES_MANAGE).await?;
    let key = body.key.trim();
    if key.is_empty()
        || !key
            .chars()
            .all(|c| c.is_ascii_lowercase() || c.is_ascii_digit() || c == '_')
    {
        return Err(DomainError::Validation(
            "key 仅允许小写字母/数字/下划线".into(),
        ));
    }
    if body.name.trim().is_empty() {
        return Err(DomainError::Validation("名称必填".into()));
    }
    // 预查重：roles.key 是主键，裸 INSERT 撞键会以 500 泄漏；
    // rows_affected 判重只对 ON CONFLICT DO NOTHING 生效，普通 INSERT 冲突必然报错
    let dup: bool = sqlx::query_scalar("SELECT EXISTS(SELECT 1 FROM roles WHERE key = $1)")
        .bind(&key)
        .fetch_one(&state.repo.db)
        .await
        .unwrap_or(false);
    if dup {
        return Err(DomainError::Validation("职务已存在".into()));
    }
    sqlx::query("INSERT INTO roles (key, name, descr, sort) VALUES ($1, $2, $3, COALESCE($4, 0))")
        .bind(key)
        .bind(body.name.trim())
        .bind(body.descr.clone())
        .bind(body.sort)
        .execute(&state.repo.db)
        .await
        .map_err(|e| DomainError::Internal(e.into()))?;
    state.repo.audit(Some(auth.id), "roles.add", None).await;
    Ok(ok(serde_json::json!({ "key": key })))
}

#[put("/admin/roles/{key}")]
async fn roles_dict_update(
    req: HttpRequest,
    state: web::Data<std::sync::Arc<AppState>>,
    path: web::Path<String>,
    body: web::Json<RoleDefReq>,
) -> DomainResult<HttpResponse> {
    let auth = staff(&req, &state).await?;
    crate::authz::require_perm(&state, &auth, crate::authz::perm::ROLES_MANAGE).await?;
    let key = path.into_inner();
    if body.name.trim().is_empty() {
        return Err(DomainError::Validation("名称必填".into()));
    }
    // key 不可改（user_roles/role_permissions 外键引用它）；改名连带刷新权限缓存
    let n = sqlx::query(
        "UPDATE roles SET name = $2, descr = $3, sort = COALESCE($4, sort) WHERE key = $1",
    )
    .bind(&key)
    .bind(body.name.trim())
    .bind(body.descr.clone())
    .bind(body.sort)
    .execute(&state.repo.db)
    .await
    .map_err(|e| DomainError::Internal(e.into()))?
    .rows_affected();
    if n == 0 {
        return Err(DomainError::NotFound(0));
    }
    crate::http::bump_guard_ver(&state).await;
    state.repo.audit(Some(auth.id), "roles.update", None).await;
    Ok(ok(serde_json::json!({ "ok": true })))
}

#[delete("/admin/roles/{key}")]
async fn roles_dict_delete(
    req: HttpRequest,
    state: web::Data<std::sync::Arc<AppState>>,
    path: web::Path<String>,
) -> DomainResult<HttpResponse> {
    let auth = staff(&req, &state).await?;
    crate::authz::require_perm(&state, &auth, crate::authz::perm::ROLES_MANAGE).await?;
    let key = path.into_inner();
    let holders: i64 = sqlx::query_scalar("SELECT count(*) FROM user_roles WHERE role_key = $1")
        .bind(&key)
        .fetch_one(&state.repo.db)
        .await
        .map_err(|e| DomainError::Internal(e.into()))?;
    if holders > 0 {
        return Err(DomainError::Validation(format!(
            "仍有 {holders} 人持有该职务，请先撤销全部授予"
        )));
    }
    let n = sqlx::query("DELETE FROM roles WHERE key = $1")
        .bind(&key)
        .execute(&state.repo.db)
        .await
        .map_err(|e| DomainError::Internal(e.into()))?
        .rows_affected();
    if n == 0 {
        return Err(DomainError::NotFound(0));
    }
    crate::http::bump_guard_ver(&state).await;
    state.repo.audit(Some(auth.id), "roles.del", None).await;
    Ok(ok(serde_json::json!({ "deleted": key })))
}

// ============ 批量发放（好学 increment-bulk.php 口径） ============
// 火花/上传量/邀请/补签卡 ×（等级多选 | 职务多选 | 指定用户），完成后群发 PM 通知。

#[derive(Deserialize)]
struct IncrementBulkReq {
    /// spark | uploaded | invite | resub_card
    kind: String,
    /// spark/invite/resub_card 为数量；uploaded 为 GB（正加负减）
    amount: i64,
    /// 目标等级（多选）；与 roles/user_ids 至少其一
    #[serde(default)]
    classes: Vec<i32>,
    /// 目标职务（多选）
    #[serde(default)]
    roles: Vec<String>,
    /// 指定用户（优先于 classes/roles）
    #[serde(default)]
    user_ids: Vec<i64>,
    /// 临时邀请有效期（天，1-365）：仅 kind=invite 时有效——直接生成 N 天到期的邀请码
    /// （好学 tmp_invites 口径）；缺省走 quota_extra 配额
    #[serde(default)]
    days: Option<i32>,
    /// PM 通知（可选；空则不发）
    #[serde(default)]
    subject: Option<String>,
    #[serde(default)]
    body: Option<String>,
    /// 发送者：self = 操作者，system = 系统私信（sender NULL）
    #[serde(default)]
    sender: Option<String>,
}

#[post("/admin/increment-bulk")]
async fn increment_bulk(
    req: HttpRequest,
    state: web::Data<std::sync::Arc<AppState>>,
    body: web::Json<IncrementBulkReq>,
) -> DomainResult<HttpResponse> {
    let auth = staff(&req, &state).await?;
    // 火花沿用 amountbonus 档；上传量/邀请/补签卡按 amountupload 档（均 99）——
    // 与旧两个工具的权限口径一致，合并页不收窄也不放宽。
    let perm = match body.kind.as_str() {
        "spark" => crate::authz::perm::USER_AMOUNTBONUS,
        "uploaded" | "invite" | "resub_card" => crate::authz::perm::USER_AMOUNTUPLOAD,
        _ => {
            return Err(DomainError::Validation(
                "kind 取值 spark/uploaded/invite/resub_card".into(),
            ))
        }
    };
    crate::authz::require_perm(&state, &auth, perm).await?;
    if body.amount == 0 {
        return Err(DomainError::Validation("数量不能为 0".into()));
    }
    // 上限口径沿用旧端点：火花 ±100 万；上传量 ±10TB（GB 换算）；邀请/补签卡 1-50
    match body.kind.as_str() {
        "spark" if body.amount.abs() > 1_000_000 => {
            return Err(DomainError::Validation("火花单次 ±1,000,000".into()));
        }
        "uploaded" if body.amount.abs() > 10 * 1024 => {
            return Err(DomainError::Validation("上传量单次 ±10TB（GB）".into()));
        }
        "invite" if body.amount.abs() > 50 => {
            return Err(DomainError::Validation("邀请单次 ±50".into()));
        }
        "invite" if body.days.is_some_and(|d| !(1..=365).contains(&d)) => {
            return Err(DomainError::Validation("临时邀请有效期 1-365 天".into()));
        }
        "invite" if body.days.is_some() && body.amount <= 0 => {
            return Err(DomainError::Validation("临时邀请数量需为正数".into()));
        }
        "resub_card" if !(1..=50).contains(&body.amount) => {
            return Err(DomainError::Validation("补签卡单次 1-50".into()));
        }
        _ => {}
    }
    if body.user_ids.is_empty() && body.classes.is_empty() && body.roles.is_empty() {
        return Err(DomainError::Validation(
            "需选择目标：等级 / 职务 / 指定用户".into(),
        ));
    }
    if body.user_ids.len() > 500 {
        return Err(DomainError::Validation("指定用户单批最多 500".into()));
    }

    let db = &state.repo.db;
    // 目标集合：指定用户优先（不叠加等级筛选，口径同 NP：receiver 直发）；
    // 否则 classes OR roles，未封禁（status<2）
    let targets: Vec<i64> = if !body.user_ids.is_empty() {
        sqlx::query_scalar("SELECT id FROM users WHERE id = ANY($1) AND status < 2")
            .bind(&body.user_ids)
            .fetch_all(db)
            .await
            .map_err(|e| DomainError::Internal(e.into()))?
    } else {
        let mut clauses: Vec<String> = Vec::new();
        if !body.classes.is_empty() {
            clauses.push("class_id = ANY($1)".into());
        }
        if !body.roles.is_empty() {
            clauses.push("id IN (SELECT user_id FROM user_roles WHERE role_key = ANY($2))".into());
        }
        let sql = format!(
            "SELECT id FROM users WHERE status < 2 AND ({}) ORDER BY id",
            clauses.join(" OR ")
        );
        sqlx::query_scalar(&sql)
            .bind(&body.classes)
            .bind(&body.roles)
            .fetch_all(db)
            .await
            .map_err(|e| DomainError::Internal(e.into()))?
    };
    if targets.is_empty() {
        return Err(DomainError::Validation("没有符合条件的目标用户".into()));
    }

    let batch_id = uuid::Uuid::new_v4().simple().to_string();
    let sender_id: Option<i64> = if body.sender.as_deref() == Some("system") {
        None
    } else {
        Some(auth.id)
    };

    // 分批执行（每批 500，避免长事务）
    let mut affected: u64 = 0;
    for chunk in targets.chunks(500) {
        match body.kind.as_str() {
            "spark" => {
                // 余额权威在流水：逐用户落 spark_ledger（负数扣减下限 0）
                for uid in chunk {
                    let idem = format!("increment_bulk:{batch_id}:{uid}");
                    if body.amount > 0 {
                        crate::economy_http::earn_spark(
                            db,
                            *uid,
                            body.amount,
                            "increment_bulk",
                            &idem,
                        )
                        .await?;
                    } else {
                        let n = sqlx::query(
                            "UPDATE users SET spark_balance = GREATEST(0, spark_balance + $2) WHERE id = $1",
                        )
                        .bind(uid)
                        .bind(body.amount)
                        .execute(db)
                        .await
                        .map_err(|e| DomainError::Internal(e.into()))?
                        .rows_affected();
                        if n > 0 {
                            let bal: i64 =
                                sqlx::query_scalar("SELECT spark_balance FROM users WHERE id = $1")
                                    .bind(uid)
                                    .fetch_one(db)
                                    .await
                                    .unwrap_or(0);
                            sqlx::query(
                                "INSERT INTO spark_ledger (id, user_id, amount, kind, idempotency_key, balance_after) \
                                 VALUES (nextval('spark_ledger_id_seq'), $1, $2, 'increment_bulk', $3, $4)",
                            )
                            .bind(uid)
                            .bind(body.amount)
                            .bind(&idem)
                            .bind(bal)
                            .execute(db)
                            .await
                            .map_err(|e| DomainError::Internal(e.into()))?;
                        }
                    }
                    affected += 1;
                }
            }
            "uploaded" => {
                let bytes = body.amount * 1024 * 1024 * 1024;
                affected += sqlx::query(
                    "UPDATE users SET uploaded = GREATEST(0, uploaded + $2) WHERE id = ANY($1)",
                )
                .bind(chunk)
                .bind(bytes)
                .execute(db)
                .await
                .map_err(|e| DomainError::Internal(e.into()))?
                .rows_affected();
            }
            "invite" => {
                if let Some(days) = body.days {
                    // 临时邀请（好学 tmp_invites 口径）：直接生成 N 天有效期的邀请码，
                    // 不走 quota_extra——码已在手，到期自动失效（status 由 worker 置 2）
                    let expires =
                        chrono::Utc::now() + chrono::Duration::days(days.clamp(1, 365) as i64);
                    for uid in chunk {
                        for _ in 0..body.amount {
                            let code = crate::domain::new_invite_code();
                            sqlx::query(
                                "INSERT INTO invites (inviter_id, code, expires_at) VALUES ($1, $2, $3)",
                            )
                            .bind(uid)
                            .bind(&code)
                            .bind(expires)
                            .execute(db)
                            .await
                            .map_err(|e| DomainError::Internal(e.into()))?;
                            affected += 1;
                        }
                    }
                } else {
                    // 普通发放：正数增发 quota_extra；负数回收（下限 0），与单用户 adjust 口径一致
                    affected += sqlx::query(
                        "UPDATE users SET quota_extra = GREATEST(0, quota_extra + $2) WHERE id = ANY($1)",
                    )
                    .bind(chunk)
                    .bind(body.amount)
                    .execute(db)
                    .await
                    .map_err(|e| DomainError::Internal(e.into()))?
                    .rows_affected();
                }
            }
            "resub_card" => {
                // 补签卡：给每人 amount 张（0 价订单入包，复用 grant-item 的背包语义）
                // 道具字典种子 kind=makeup_card；gaps_http 补签流程引用 resub_card（历史不一致），
                // 此处两种都认，优先 makeup_card
                let item: Option<(i64, serde_json::Value)> = sqlx::query_as(
                    "SELECT id, config FROM shop_items WHERE kind IN ('makeup_card','resub_card') AND active = true ORDER BY kind = 'makeup_card' DESC, id LIMIT 1",
                )
                .fetch_optional(db)
                .await
                .map_err(|e| DomainError::Internal(e.into()))?;
                let Some((item_id, config)) = item else {
                    return Err(DomainError::Validation(
                        "商店缺少可用的补签卡道具（makeup_card）".into(),
                    ));
                };
                for uid in chunk {
                    for _ in 0..body.amount {
                        let idem = format!(
                            "increment_bulk:{batch_id}:{uid}:{}",
                            uuid::Uuid::new_v4().simple()
                        );
                        sqlx::query(
                            "INSERT INTO shop_orders (user_id, item_id, price, idempotency_key, config_snapshot) \
                             VALUES ($1, $2, 0, $3, $4)",
                        )
                        .bind(uid)
                        .bind(item_id)
                        .bind(&idem)
                        .bind(&config)
                        .execute(db)
                        .await
                        .map_err(|e| DomainError::Internal(e.into()))?;
                        affected += 1;
                    }
                }
            }
            _ => unreachable!(),
        }

        // PM 通知（可选）
        if let (Some(subject), Some(text)) = (body.subject.as_deref(), body.body.as_deref()) {
            if !subject.trim().is_empty() && !text.trim().is_empty() {
                for uid in chunk {
                    sqlx::query(
                        "INSERT INTO messages (sender_id, receiver_id, subject, body) VALUES ($1, $2, $3, $4)",
                    )
                    .bind(sender_id)
                    .bind(uid)
                    .bind(subject.trim())
                    .bind(text.trim())
                    .execute(db)
                    .await
                    .map_err(|e| DomainError::Internal(e.into()))?;
                }
            }
        }
    }

    state
        .repo
        .audit(
            Some(auth.id),
            &format!("increment_bulk.{}", body.kind),
            None,
        )
        .await;
    Ok(ok(serde_json::json!({
        "affected": affected as i64,
        "targets": targets.len() as i64,
        "kind": body.kind,
        "amount": body.amount,
    })))
}

pub fn mount_p3_tools(scope: actix_web::Scope) -> actix_web::Scope {
    scope
        .service(torrent_batch)
        .service(tags_dict_list)
        .service(tags_dict_add)
        .service(tags_dict_update)
        .service(tags_dict_delete)
        .service(hr_records)
        .service(hr_batch_pardon)
        .service(admin_invites)
        .service(admin_attendance)
        .service(admin_attendance_makeup)
        .service(admin_attendance_makeup_delete)
        .service(admin_rename_logs)
        .service(admin_modify_logs)
        .service(admin_medals)
        .service(admin_medal_add)
        .service(admin_medal_update)
        .service(admin_medal_delete)
        .service(admin_user_medals)
        .service(admin_user_medal_revoke)
        .service(admin_shop_items)
        .service(admin_shop_item_add)
        .service(admin_shop_item_update)
        .service(admin_shop_item_delete)
        .service(admin_user_props)
        .service(admin_user_prop_revoke)
        .service(admin_users_batch)
        .service(admin_ban_delete_by_ip)
        .service(admin_template_create)
        .service(admin_template_delete)
        .service(section_modes_list)
        .service(section_mode_add)
        .service(section_mode_update)
        .service(section_mode_delete)
        .service(section_dict_admin)
        .service(section_dict_public)
        .service(section_dict_add)
        .service(section_dict_update)
        .service(section_dict_delete)
        .service(category_flags)
        .service(jixiao_types_list)
        .service(jixiao_type_add)
        .service(jixiao_type_update)
        .service(jixiao_type_delete)
        .service(tasks_list)
        .service(task_add)
        .service(task_update)
        .service(task_delete)
        .service(tracker_urls_list)
        .service(tracker_url_add)
        .service(tracker_url_update)
        .service(tracker_url_delete)
        .service(admin_user_rename)
        .service(roles_dict_list)
        .service(roles_dict_add)
        .service(roles_dict_update)
        .service(roles_dict_delete)
        .service(increment_bulk)
}
