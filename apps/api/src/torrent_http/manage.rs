//! 种子管理（M02/M03）：编辑/定价/恢复/重提/删除。
//! 从 torrent_http.rs 按域拆出。

use actix_web::{delete, post, put, web, HttpRequest, HttpResponse, Responder};

use crate::dto::ok;
use crate::errors::{DomainError, DomainResult};
use crate::http::require_auth;
use crate::state::AppState;
use crate::torrents;

use super::interact::TorrentEditReq;

#[put("/torrents/{id}")]
async fn edit_torrent(
    req: HttpRequest,
    state: web::Data<std::sync::Arc<AppState>>,
    path: web::Path<i64>,
    body: web::Json<TorrentEditReq>,
) -> DomainResult<HttpResponse> {
    let auth = require_auth(&req, &state).await?;
    let tid = path.into_inner();
    // 0150：IMDB 直填暂存（TorrentEdit.imdb_id 是 &str，借本地变量）
    let imdb_norm: Option<String> =
        body.imdb_id.as_deref().map(str::trim).and_then(|s| {
            // 空串 = 清空；非空统一大写后校验 TT+7~8 位；非法回落 None
            let up = s.to_ascii_uppercase();
            let valid = up.len() >= 9
                && up.starts_with("TT")
                && up[2..].chars().all(|c| c.is_ascii_digit());
            if s.is_empty() || valid {
                Some(up)
            } else {
                None
            }
        });
    // 0170：返回 true = 编辑后保持原审核状态（staff 管理通道或 ≥ 免审等级作者）
    let kept = torrents::edit_torrent(
        &state.repo.db,
        tid,
        (auth.id, auth.class_id as i16),
        &torrents::TorrentEdit {
            name: body
                .name
                .as_deref()
                .map(str::trim)
                .filter(|s| !s.is_empty()),
            small_descr: body.small_descr.as_deref(),
            descr: body.descr.as_deref(),
            anonymous: body.anonymous,
            category_id: body.category_id,
            medium_id: body.medium_id,
            grade_id: body.grade_id,
            edition_id: body.edition_id,
            imdb_id: imdb_norm.as_deref(),
            // 0173：封面外链（None=不动 / Some("")=清除 / Some(url)=写入）
            poster: body.poster.as_deref().map(str::trim),
            // 0173：MediaInfo 全文（同三态语义）
            mediainfo: body.mediainfo.as_deref().map(str::trim),
        },
    )
    .await?;
    // 标签整组编辑（0159 P1）：Some([...]) = 同步为该组（先清后打），
    // Some([]) = 清空，None = 不动。校验/官种联动与发布、详情页同源。
    if let Some(tag_ids) = &body.tag_ids {
        let owner: Option<i64> =
            sqlx::query_scalar("SELECT owner_id FROM torrents WHERE id = $1")
                .bind(tid)
                .fetch_optional(&state.repo.db)
                .await
                .map_err(|e| DomainError::Internal(e.into()))?;
        if owner.is_none() {
            return Err(DomainError::NotFound(tid));
        }
        if auth.class_id < 90 && owner != Some(auth.id) {
            return Err(DomainError::Forbidden);
        }
        sqlx::query("DELETE FROM tags WHERE torrent_id = $1")
            .bind(tid)
            .execute(&state.repo.db)
            .await
            .map_err(|e| DomainError::Internal(e.into()))?;
        torrents::apply_torrent_tags(
            &state.repo.db,
            tid,
            tag_ids,
            (auth.id, auth.class_id as i16),
        )
        .await?;
    }
    state
        .repo
        .audit(Some(auth.id), "torrent.edit", Some(tid))
        .await;
    // 2.5 详情对象缓存：编辑即失效（descr/分类等共享段字段变了）
    super::aggregate::invalidate_tdetail_cache(&state, tid).await;
    // 多维质量（0087 同发布表单）：Some(map) = 整组重建（白名单 + 字典归属校验后
    // 先清后写；未含的旧维删除）。校验口径与 upload_sections 同源。
    if let Some(map) = &body.sections {
        let owner: Option<i64> =
            sqlx::query_scalar("SELECT owner_id FROM torrents WHERE id = $1")
                .bind(tid)
                .fetch_optional(&state.repo.db)
                .await
                .map_err(|e| DomainError::Internal(e.into()))?;
        if owner.is_none() {
            return Err(DomainError::NotFound(tid));
        }
        if auth.class_id < 90 && owner != Some(auth.id) {
            return Err(DomainError::Forbidden);
        }
        for (kind, dict_id) in map {
            if !crate::admin_p3_http::is_custom_kind(&state.repo.db, kind)
                .await
            {
                return Err(DomainError::Validation(format!(
                    "未知维度 {kind}"
                )));
            }
            let ok: bool = sqlx::query_scalar(
                "SELECT EXISTS(SELECT 1 FROM \
                     section_dict WHERE id = $2 AND kind = $1)",
            )
            .bind(kind)
            .bind(dict_id)
            .fetch_one(&state.repo.db)
            .await
            .map_err(|e| DomainError::Internal(e.into()))?;
            if !ok {
                return Err(DomainError::Validation(format!(
                    "维度 {kind} 的字典项 {dict_id} 不存在"
                )));
            }
        }
        sqlx::query("DELETE FROM torrent_sections WHERE torrent_id = $1")
            .bind(tid)
            .execute(&state.repo.db)
            .await
            .map_err(|e| DomainError::Internal(e.into()))?;
        for (kind, dict_id) in map {
            sqlx::query(
                "INSERT INTO torrent_sections \
                 (torrent_id, kind, dict_id) VALUES ($1, $2, $3)",
            )
            .bind(tid)
            .bind(kind)
            .bind(dict_id)
            .execute(&state.repo.db)
            .await
            .map_err(|e| DomainError::Internal(e.into()))?;
        }
        // sections 是这一轮的真值源：按名称反向落旧三列（维度已被站长删除的列不动），
        // 否则「改了学段但按学段筛搜不到」会继续存在
        crate::torrents::sync_legacy_columns(&state.repo.db, tid).await?;
    }
    // 0148 C1：编辑 descr 后重提取 IMDB（descr 提取得到才覆盖，否则保留旧值）
    if let Some(d) = body.descr.as_deref() {
        if let Some(imdb) = crate::publish_http::extract_imdb_pub(d) {
            let _ = sqlx::query(
                "UPDATE torrents SET imdb_id = $2 WHERE id = $1 AND \
                 (imdb_id IS NULL OR imdb_id <> $2)",
            )
            .bind(tid)
            .bind(&imdb)
            .execute(&state.repo.db)
            .await;
        }
        // 0159 封面回落与编辑同步：descr 变了，简介首图可能也变——仅在
        // 存量 poster 为空时回落写入（用户显式填过的封面不被动覆盖）。
        // poster 值走 jsonb_build_object 参数化（拼接 jsonb 字面量遇 URL
        // 特殊字符会 22P02，且是注入面）。
        if let Some(poster) = crate::publish_http::first_descr_image_pub(Some(d)) {
            let _ = sqlx::query(
                "UPDATE torrents SET media_info = COALESCE(media_info, '{}'::jsonb) \
                 || jsonb_build_object('poster', $2) \
                 WHERE id = $1 AND COALESCE(media_info->>'poster', '') = ''",
            )
            .bind(tid)
            .bind(&poster)
            .execute(&state.repo.db)
            .await;
        }
    }
    Ok(ok(serde_json::json!({
        "edited": true,
        // 0170：是否回退待审（staff/免审等级编辑为 false，种子不出列表）
        "requeued": !kept,
        "note": if kept { "已保存" } else { "已回退待审核" },
    })))
}

/// 修改付费定价（0086）：发布者或 staff；改价不影响已购（以 torrent_purchases 已扣为准）
#[derive(serde::Deserialize)]
struct PriceReq {
    price: i64,
}

#[put("/torrents/{id}/price")]
async fn set_torrent_price(
    req: HttpRequest,
    state: web::Data<std::sync::Arc<AppState>>,
    path: web::Path<i64>,
    body: web::Json<PriceReq>,
) -> DomainResult<HttpResponse> {
    let auth = require_auth(&req, &state).await?;
    let id = path.into_inner();
    let owner: Option<i64> =
        sqlx::query_scalar("SELECT owner_id FROM torrents WHERE id = $1")
            .bind(id)
            .fetch_optional(&state.repo.db)
            .await
            .map_err(|e| DomainError::Internal(e.into()))?;
    let Some(owner_id) = owner else {
        return Err(DomainError::NotFound(id));
    };
    if owner_id != auth.id && auth.class_id < 90 {
        return Err(DomainError::Forbidden);
    }
    let price = body.price.clamp(0, 1_000_000);
    sqlx::query("UPDATE torrents SET price = $2 WHERE id = $1")
        .bind(id)
        .bind(price)
        .execute(&state.repo.db)
        .await
        .map_err(|e| DomainError::Internal(e.into()))?;
    state
        .repo
        .audit(Some(auth.id), "torrent.price_set", Some(id))
        .await;
    super::aggregate::invalidate_tdetail_cache(&state, id).await;
    Ok(ok(serde_json::json!({ "price": price })))
}

/// 恢复软删种子（approval_status 3 → 0 待审）：此前误删后只能直连数据库手工修数
#[post("/torrents/{id}/restore")]
async fn restore_torrent(
    req: HttpRequest,
    state: web::Data<std::sync::Arc<AppState>>,
    path: web::Path<i64>,
) -> DomainResult<impl Responder> {
    let auth = require_auth(&req, &state).await?;
    crate::authz::require_perm(
        &state,
        &auth,
        crate::authz::perm::TORRENT_MANAGE,
    )
    .await?;
    let id = path.into_inner();
    torrents::restore_torrent(&state.repo.db, id).await?;
    state
        .repo
        .audit(Some(auth.id), "torrent.restore", Some(id))
        .await;
    super::aggregate::invalidate_tdetail_cache(&state, id).await;
    Ok(ok(serde_json::json!({ "restored": id })))
}

/// 被拒种子修改后重提（U4 §12.5 审核闭环：rejected → pending，保留原 id 与评论区）。
/// 仅作者本人；清拒绝标记（deny_reason/note），重进审核队列。
#[post("/torrents/{id}/resubmit")]
async fn resubmit_torrent(
    req: HttpRequest,
    state: web::Data<std::sync::Arc<AppState>>,
    path: web::Path<i64>,
) -> DomainResult<impl Responder> {
    let auth = require_auth(&req, &state).await?;
    let id = path.into_inner();
    let owner: Option<(i64, i16)> = sqlx::query_as(
        "SELECT owner_id, approval_status FROM torrents WHERE id = $1",
    )
    .bind(id)
    .fetch_optional(&state.repo.db)
    .await
    .map_err(|e| DomainError::Internal(e.into()))?;
    let Some((owner_id, status)) = owner else {
        return Err(DomainError::NotFound(id));
    };
    if owner_id != auth.id {
        return Err(DomainError::Forbidden);
    }
    if status != 2 {
        return Err(DomainError::Validation(
            "仅被拒种子可重提（当前状态不符）".into(),
        ));
    }
    let n = sqlx::query(
        "UPDATE torrents SET approval_status = 0, deny_reason_id = NULL, deny_note = NULL \
         WHERE id = $1 AND approval_status = 2",
    )
    .bind(id)
    .execute(&state.repo.db)
    .await
    .map_err(|e| DomainError::Internal(e.into()))?
    .rows_affected();
    if n == 0 {
        return Err(DomainError::Validation("状态已变化，请刷新".into()));
    }
    state
        .repo
        .audit(Some(auth.id), "torrent.resubmit", Some(id))
        .await;
    super::aggregate::invalidate_tdetail_cache(&state, id).await;
    Ok(ok(serde_json::json!({ "resubmitted": id })))
}

/// 删除种子（软删 approval_status=3；staff 任意删，作者仅限未过审）
#[delete("/torrents/{id}")]
async fn delete_torrent(
    req: HttpRequest,
    state: web::Data<std::sync::Arc<AppState>>,
    path: web::Path<i64>,
) -> DomainResult<HttpResponse> {
    let auth = require_auth(&req, &state).await?;
    let id = path.into_inner();
    torrents::delete_torrent(
        &state.repo.db,
        id,
        (auth.id, auth.class_id as i16),
    )
    .await?;
    state
        .repo
        .audit(Some(auth.id), "torrent.delete", Some(id))
        .await;
    super::aggregate::invalidate_tdetail_cache(&state, id).await;
    Ok(ok(serde_json::json!({ "deleted": id })))
}
