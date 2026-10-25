//! 种子管理（M02/M03）：编辑/定价/恢复/重提。
//! 从 torrent_http.rs 按域拆出。

use actix_web::{post, put, web, HttpRequest, HttpResponse, Responder};

use crate::dto::ok;
use crate::errors::{DomainError, DomainResult};
use crate::http::require_auth;
use crate::state::AppState;
use crate::torrents;

use super::interact::TorrentEditReq;

#[utoipa::path(
    put,
    path = "/api/v1/torrents/{id}",
    tag = "种子",
    description = "编辑种子（发布者本人或持编辑权；改 info dict 会动 info_hash，\
            故被同种判为重复——这是刻意口径）。",
    params(
        ("id" = i64, description = "种子 id"),
    ),
    responses(
        (status = 200, description = "ok"),
        (status = 401, description = "会话未登录（code 2001）"),
        (status = 404, description = "种子不存在或不可见（code 1004）"),
        (status = 422, description = "参数校验失败（code 1002）"),
        (status = 403, description = "权限不足（code 2003）"),
    )
)]
#[put("/torrents/{id}")]
pub(crate) async fn edit_torrent(
    req: HttpRequest,
    state: web::Data<std::sync::Arc<AppState>>,
    path: web::Path<i64>,
    body: web::Json<TorrentEditReq>,
) -> DomainResult<HttpResponse> {
    let auth = require_auth(&req, &state).await?;
    let tid = path.into_inner();
    // 0150：IMDB 直填三态——未填 = 不动，空串 = 清空，非空必须解析得出
    // 编号（也接受整条 IMDb 链接），与发种口同一把正则（extract_imdb_pub）。
    // 此前非法形态被静默回落成 None，也就是「保持原值」，接口照样回 200 ⇒
    // 用户手滑填了 `abc` 以为保存成功（2026-10-10 实测）。
    let raw_imdb = body.imdb_id.as_deref().map(str::trim);
    let imdb_norm: Option<String> = match raw_imdb {
        None => None,
        Some("") => Some(String::new()),
        Some(raw) => Some(
            crate::publish_http::extract_imdb_pub(raw).ok_or_else(|| {
                DomainError::Validation(crate::publish_http::IMDB_HINT.into())
            })?,
        ),
    };
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
            nfo: body.nfo.as_deref().map(str::trim),
        },
    )
    .await?;
    // P-346：MediaInfo 一变就重跑一致性判定（站方 93f741b5 的机器抓标题党）。
    // 此前判定器全仓零调用点——MediaInfo 原样存下就没人判过，`mediainfo_check`
    // 恒为 NULL（2026-10-16 实测：编辑口写入 392 字符后该列仍是 NULL）。
    //
    // 两个口径：
    // · 名字跟着标题走——编辑同时改了 name 时，声明值按库里的现标题重算，
    //   否则「改完标题就不再算标题党」会被当成漏洞。
    // · 判定是提示不是闸门——解析失败/库写失败都不许打断编辑回包，
    //   与站方「不拦截发布」的判断同口径，故此处刻意吞掉错误。
    if let Some(t) = body
        .mediainfo
        .as_deref()
        .map(str::trim)
        .filter(|s| !s.is_empty())
    {
        let cur_name: Option<String> =
            sqlx::query_scalar("SELECT name FROM torrents WHERE id = $1")
                .bind(tid)
                .fetch_optional(&state.repo.db)
                .await
                .ok()
                .flatten();
        if let Some(nm) = cur_name {
            let _ = crate::mediainfo_service::check_and_store(
                &state,
                tid,
                &nm,
                Some(t),
            )
            .await;
        }
    }
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
    // 多维属性（B2 六类型，0195）：Some(值) = 整组重建（未含的旧维删除，
    // 与前端「整表单保存」语义一致）。校验/写入与发种口**同源**
    // （`publish_http::upload_sections`），六类型 + 必填 + 多值约束一致生效。
    if let Some(raw) = &body.sections {
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
        let json = serde_json::to_string(raw)
            .map_err(|e| DomainError::Internal(e.into()))?;
        let parsed = crate::publish_http::upload_sections::parse_sections(
            &state.repo.db,
            Some(&json),
        )
        .await?;
        crate::publish_http::upload_sections::write_sections(
            &state.repo.db,
            tid,
            &parsed,
            true, // 编辑口径：未提交的维度视为清空
        )
        .await?;
        // sections 是这一轮的真值源：按名称反向落旧三列（维度已被站长删除的列不动），
        // 否则「改了学段但按学段筛搜不到」会继续存在
        crate::torrents::sync_legacy_columns(&state.repo.db, tid).await?;
        // log_score/haslog 是派生维度（b9a3e90 起由 torrent_logs 回流）：
        // 编辑表单不带它们，clear_absent=true 会把回流行删掉——
        // 编辑后重算一遍（幂等），筛选面不因常规编辑而失效
        crate::torrents::sync_log_dims(&state.repo.db, tid).await?;
    }
    // 推荐位（0184 编辑对齐发布页，NP 挑选口径）：staff 专属；
    // 任一字段出现即整组覆写，校验与 upload_files_promo 同源
    if body.pos_state.is_some()
        || body.pick_type.is_some()
        || body.pos_state_until.is_some()
    {
        if auth.class_id < 90 {
            return Err(DomainError::Forbidden); // 置顶/推荐仅管理组
        }
        let pos = body.pos_state.unwrap_or(0);
        if ![0, 1, 2].contains(&pos) {
            return Err(DomainError::Validation("置顶位置取值 0/1/2".into()));
        }
        let pick = body.pick_type.unwrap_or(0);
        if ![0, 1, 2].contains(&pick) {
            return Err(DomainError::Validation("推荐影片取值 0/1/2".into()));
        }
        let until = body
            .pos_state_until
            .as_deref()
            .map(str::trim)
            .filter(|s| !s.is_empty())
            .map(|s| {
                chrono::DateTime::parse_from_rfc3339(s)
                    .map_err(|_| {
                        DomainError::Validation("置顶截止时间格式无效".into())
                    })
                    .map(|dt| dt.with_timezone(&chrono::Utc))
            })
            .transpose()?;
        sqlx::query(
            "UPDATE torrents SET pos_state = $2, \
                 pos_state_until = $3, pick_type = $4, \
                 mtime = now() WHERE id = $1",
        )
        .bind(tid)
        .bind(pos)
        .bind(until)
        .bind(pick)
        .execute(&state.repo.db)
        .await
        .map_err(|e| DomainError::Internal(e.into()))?;
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
        if let Some(poster) =
            crate::publish_http::first_descr_image_pub(Some(d))
        {
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
#[derive(serde::Deserialize, utoipa::ToSchema)]
struct PriceReq {
    price: i64,
}

#[utoipa::path(
    put,
    path = "/api/v1/torrents/{id}/price",
    tag = "种子",
    description = "设置付费价格（发布者本人）。",
    params(
        ("id" = i64, description = "种子 id"),
    ),
    responses(
        (status = 200, description = "ok"),
        (status = 401, description = "会话未登录（code 2001）"),
        (status = 404, description = "种子不存在或不可见（code 1004）"),
        (status = 422, description = "参数校验失败（code 1002）"),
        (status = 403, description = "权限不足（code 2003）"),
    )
)]
#[put("/torrents/{id}/price")]
pub(crate) async fn set_torrent_price(
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
    // P-3：改价口与发种口同一闸门（paid_torrent_enabled / max_price 此前
    // 零读取方，站长关掉付费后作者仍可用本口把免费种改成付费种）。
    // 越界明确报错而非 clamp——与发种口「静默改写即缺陷」同口径。
    let gate =
        crate::publish_http::upload_precheck_pub::price_gate(&state.repo.db)
            .await;
    if body.price != 0 && !gate.enabled {
        return Err(DomainError::Validation(
            "本站未开启付费种子：价格只能为 0（免费）".into(),
        ));
    }
    if body.price > gate.limit {
        return Err(DomainError::Validation(format!(
            "付费下载价格需在 0–{} 之间（站点售价上限）",
            gate.limit
        )));
    }
    let price = body.price;
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
#[utoipa::path(
    post,
    path = "/api/v1/torrents/{id}/restore",
    tag = "种子",
    description = "回收站恢复（软删种置回通过态）。",
    params(
        ("id" = i64, description = "种子 id"),
    ),
    responses(
        (status = 200, description = "ok"),
        (status = 401, description = "会话未登录（code 2001）"),
        (status = 404, description = "种子不存在或不可见（code 1004）"),
        (status = 422, description = "参数校验失败（code 1002）"),
        (status = 403, description = "权限不足（code 2003）"),
    )
)]
#[post("/torrents/{id}/restore")]
pub(crate) async fn restore_torrent(
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
    // 恢复回待审（0）重新进 tracker 白名单，同样立即刷新（P2-2）
    crate::http::bump_guard_ver(&state).await;
    super::aggregate::invalidate_tdetail_cache(&state, id).await;
    Ok(ok(serde_json::json!({ "restored": id })))
}

/// 被拒种子修改后重提（U4 §12.5 审核闭环：rejected → pending，保留原 id 与评论区）。
/// 仅作者本人；清拒绝标记（deny_reason/note），重进审核队列。
#[utoipa::path(
    post,
    path = "/api/v1/torrents/{id}/resubmit",
    tag = "种子",
    description = "重新提交审核（下架/驳回种走这条路回到审核台）。",
    params(
        ("id" = i64, description = "种子 id"),
    ),
    responses(
        (status = 200, description = "ok"),
        (status = 401, description = "会话未登录（code 2001）"),
        (status = 404, description = "种子不存在或不可见（code 1004）"),
        (status = 422, description = "参数校验失败（code 1002）"),
        (status = 403, description = "权限不足（code 2003）"),
    )
)]
#[post("/torrents/{id}/resubmit")]
pub(crate) async fn resubmit_torrent(
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
    // 审计 2026-10-08 P2-1：重提此前不要求任何实际修改，被拒种可原样重提
    // 反复占用审核队列。加 10 分钟冷却（以 mtime 计，编辑/裁决都会刷新它），
    // 留出「看完拒因→修改→重提」的正常节奏，只挡机械刷队列。
    let mtime: Option<chrono::DateTime<chrono::Utc>> =
        sqlx::query_scalar("SELECT mtime FROM torrents WHERE id = $1")
            .bind(id)
            .fetch_optional(&state.repo.db)
            .await
            .map_err(|e| DomainError::Internal(e.into()))?
            .flatten();
    if let Some(mt) = mtime {
        if chrono::Utc::now() - mt < chrono::Duration::minutes(10) {
            return Err(DomainError::Validation(
                "重提冷却中（被拒后请至少 10 分钟后再试，期间可修改内容）"
                    .into(),
            ));
        }
    }
    let n = sqlx::query(
        "UPDATE torrents SET approval_status = 0, \
         approved_at = NULL, deny_reason_id = NULL, deny_note = NULL \
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
