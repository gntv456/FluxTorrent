//! P1-1 种子批量工作台（参考站 torrent/torrents 批量动作口径）
//! 从 admin_p3_http.rs 按域拆出。

use actix_web::{post, web, HttpRequest, HttpResponse};
use serde::Deserialize;

use crate::dto::ok;
use crate::errors::{DomainError, DomainResult};
use crate::state::AppState;

use super::{check_ids, staff};

// ============ P1-1 种子批量工作台（参考站 torrent/torrents 批量动作口径） ============

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

const PROMO_KINDS: [&str; 6] =
    ["free", "x2", "x2free", "half", "x2half", "p30"];

#[post("/admin/torrents/batch")]
async fn torrent_batch(
    req: HttpRequest,
    state: web::Data<std::sync::Arc<AppState>>,
    body: web::Json<TorrentBatchReq>,
) -> DomainResult<HttpResponse> {
    let auth = staff(&req, &state).await?;
    crate::authz::require_perm(
        &state,
        &auth,
        crate::authz::perm::TORRENT_MANAGE,
    )
    .await?;
    let ids = &body.ids;
    check_ids(ids)?;
    let db = &state.repo.db;
    let id_arr = ids.to_vec();
    let n: u64 = match body.action.as_str() {
        "sticky" => {
            let ps = body.pos_state.unwrap_or(1);
            if !(0..=1).contains(&ps) {
                return Err(DomainError::Validation(
                    "pos_state 取值 0/1/2".into(),
                ));
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
            let until = body.promo_until.unwrap_or_else(|| {
                chrono::Utc::now() + chrono::Duration::hours(48)
            });
            sqlx::query(
                "DELETE FROM promotions WHERE scope = \
                 'torrent' AND torrent_id = ANY($1) AND source = 'manual'",
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
                return Err(DomainError::Validation(
                    "pick_type 取值 0/1/2".into(),
                ));
            }
            sqlx::query(
                "UPDATE torrents SET pick_type = $2, \
             mtime = now() WHERE id = ANY($1)",
            )
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
            // 0159：批量打标与发布/详情同口径（scope/enabled/official 校验，
            // official_tag 物化列联动）——此前直插绕过全部校验
            for tid in &id_arr {
                crate::torrents::apply_torrent_tags(
                    db,
                    *tid,
                    &body.tag_ids,
                    (auth.id, auth.class_id as i16),
                )
                .await?;
            }
            id_arr.len() as u64
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
            sqlx::query(
                "UPDATE torrents SET hr_policy = $2, \
             mtime = now() WHERE id = ANY($1)",
            )
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
        "change_category" => {
            // 四个字段全 COALESCE：全 None 时 SQL 仍会命中 N 行并回报「成功 N 条」，
            // 但一个字段都没改——先挡掉空提交，别让操作者以为生效了
            if body.category_id.is_none()
                && body.medium_id.is_none()
                && body.grade_id.is_none()
                && body.edition_id.is_none()
            {
                return Err(DomainError::Validation("缺少要改的分类字段".into()));
            }
            sqlx::query(
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
            .rows_affected()
        }
        "change_sections" => {
            if body.sections.is_empty() {
                return Err(DomainError::Validation("缺少维度取值".into()));
            }
            // 先全量校验再写：外键只保证 dict_id 那行存在，不保证它属于该 kind，
            // 缺这一步可以把「编码=x264」挂到「学段」维度下（与编辑口同口径）
            for (kind, dict_id) in &body.sections {
                if !crate::admin_p3_http::is_custom_kind(db, kind).await {
                    return Err(DomainError::Validation(format!(
                        "未知维度 {kind}"
                    )));
                }
                let ok: bool = sqlx::query_scalar(
                    "SELECT EXISTS(SELECT 1 FROM section_dict WHERE id = $1 \
                     AND kind = $2)",
                )
                .bind(dict_id)
                .bind(kind)
                .fetch_one(db)
                .await
                .map_err(|e| DomainError::Internal(e.into()))?;
                if !ok {
                    return Err(DomainError::Validation(format!(
                        "维度 {kind} 的字典项 {dict_id} 不存在"
                    )));
                }
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
        "delete" => sqlx::query(
            "UPDATE torrents SET approval_status = 3, \
             mtime = now() WHERE id = ANY($1)",
        )
        .bind(&id_arr)
        .execute(db)
        .await
        .map_err(|e| DomainError::Internal(e.into()))?
        .rows_affected(),
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
    // 批量动作改的是列表可见内容（分类/状态/统计）：推进列表缓存代际
    if n > 0 {
        crate::torrent_http::bump_list_cache_gen(&state).await;
    }
    Ok(ok(serde_json::json!({ "affected": n as i64 })))
}
