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
    /// 维度取值：kind → 值（change_sections 动作）。
    /// 值支持旧格式整数（枚举单选）与新格式对象（B2 六类型，详见
    /// `publish_http::upload_sections`）。
    #[serde(default)]
    sections: serde_json::Value,
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
            // 深测四轮（2026-10-03）：promo_kind 未给才缺省 free；给了但不在
            // 白名单的值直接 400——旧版 unwrap_or 会把拼错的 kind（如 "x2free"
            // 误传 "2xfree"）静默挂成 free，管理端无感知（本轮深测实测踩中）。
            let kind = match body.promo_kind.as_deref() {
                None => "free",
                Some(k) if PROMO_KINDS.contains(&k) => k,
                Some(k) => {
                    return Err(DomainError::Validation(format!(
                        "未知促销类型：{k}（合法值：{PROMO_KINDS:?}）"
                    )));
                }
            };
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
            // 深测 2026-10-03：挂 H&R 时把站点缺省口径（hr_hours 设定；days 缺省 14）
            // 落进种子 policy —— worker 快照与详情页展示都优先读种子 policy，
            // 只写 {"on":true} 会让策略全靠 COALESCE 兜底，后台改 hr_hours 不生效。
            let policy: serde_json::Value = if on {
                let hr_hours: i32 = sqlx::query_scalar(
                    "SELECT COALESCE(NULLIF((SELECT value FROM site_settings \
                     WHERE name = 'hr_hours'), '')::int, 48)",
                )
                .fetch_one(db)
                .await
                .unwrap_or(48)
                .clamp(1, 24 * 365);
                serde_json::json!({ "on": true, "seed_hours": hr_hours, "days": 14 })
            } else {
                serde_json::Value::Null
            };
            sqlx::query(
                "UPDATE torrents SET hr_policy = $2, \
             mtime = now() WHERE id = ANY($1)",
            )
            .bind(&id_arr)
            .bind(policy)
            .execute(db)
            .await
            .map_err(|e| DomainError::Internal(e.into()))?
            .rows_affected()
        }
        "change_category" => {
            // 四审 L3 P0：旧三列不是批量写入口。直改 medium/grade/edition 不会反写
            // torrent_sections，改完详情页与按维度筛选仍显旧值——同一份数据两个真值。
            // 维度取值一律走 change_sections，由 sync_legacy_columns 单向落列。
            if body.medium_id.is_some()
                || body.grade_id.is_some()
                || body.edition_id.is_some()
            {
                return Err(DomainError::Validation(
                    "媒介/学段/版本请用 change_sections 提交维度取值（旧三列不再单独可写）"
                        .into(),
                ));
            }
            // 全 COALESCE 的旧写法在全 None 时仍会命中 N 行并回报「成功 N 条」，
            // 一个字段都没改——这里要求显式给出目标分类。
            let Some(category_id) = body.category_id else {
                return Err(DomainError::Validation("缺少要改的分类".into()));
            };
            sqlx::query(
                "UPDATE torrents SET category_id = $2, mtime = now() \
                 WHERE id = ANY($1)",
            )
            .bind(&id_arr)
            .bind(category_id)
            .execute(db)
            .await
            .map_err(|e| crate::errors::db_to_domain(e, "分类"))?
            .rows_affected()
        }
        "change_sections" => {
            // 与单条编辑口同源（六类型 + 多值 + 字典归属），但用**部分更新**口径：
            // 批量选择里各种子既有维度各不相同，编辑口的整组重建会把没勾的维度
            // 全清掉；这里只动给定维度（批量 UI 一次改一维）。
            let given = match body.sections.as_object() {
                Some(o) if !o.is_empty() => o.clone(),
                _ => {
                    return Err(DomainError::Validation("缺少维度取值".into()))
                }
            };
            let json = serde_json::to_string(&body.sections)
                .map_err(|e| DomainError::Internal(e.into()))?;
            let parsed =
                crate::publish_http::upload_sections::parse_sections_ex(
                    db,
                    Some(&json),
                    true,
                )
                .await?;
            // 给了但解析为空（如 {"dict_ids":[]}）＝清空该维度
            let cleared: Vec<String> = given
                .keys()
                .filter(|k| !parsed.iter().any(|s| &s.kind == *k))
                .cloned()
                .collect();
            let mut n: u64 = 0;
            for tid in &id_arr {
                crate::publish_http::upload_sections::write_sections(
                    db, *tid, &parsed, false,
                )
                .await?;
                if !cleared.is_empty() {
                    sqlx::query(
                        "DELETE FROM torrent_sections WHERE torrent_id = $1 \
                         AND kind = ANY($2)",
                    )
                    .bind(*tid)
                    .bind(&cleared)
                    .execute(db)
                    .await
                    .map_err(|e| DomainError::Internal(e.into()))?;
                }
                crate::torrents::sync_legacy_columns(db, *tid).await?;
                n += 1;
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
