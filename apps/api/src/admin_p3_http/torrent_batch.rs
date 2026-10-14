//! P1-1 种子批量工作台（参考站 torrent/torrents 批量动作口径）
//! 从 admin_p3_http.rs 按域拆出。

use actix_web::{post, web, HttpRequest, HttpResponse};
use serde::Deserialize;
use std::collections::HashMap;

use crate::dto::ok;
use crate::errors::{DomainError, DomainResult};
use crate::state::AppState;

use super::{check_ids, staff};

#[derive(Deserialize)]
struct TorrentBatchReq {
    action: String,
    ids: Vec<i64>,
    #[serde(default)] // sticky: 0 普通 / 1 置顶
    pos_state: Option<i16>,
    #[serde(default)]
    pos_state_until: Option<chrono::DateTime<chrono::Utc>>,
    #[serde(default)] // promo: free/x2/x2free/half/x2half/p30
    promo_kind: Option<String>,
    #[serde(default)]
    promo_until: Option<chrono::DateTime<chrono::Utc>>,
    #[serde(default)] // recommend: 0 普通 / 1 推荐 / 2 经典
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
    /// 维度取值：kind → 值（change_sections）。旧格式整数/新格式对象
    /// （B2 六类型，详见 `publish_http::upload_sections`）。
    #[serde(default)]
    sections: serde_json::Value,
    /// 乐观并发（F2 / FRAME §5，仅 hr/unhr 消费）：torrent_id → 读到的
    /// mtime 不一致 → 409(5003)；缺省/未携带不校验（灰度，同 expected.rs）。
    #[serde(default)]
    expected_mtime: Option<HashMap<i64, String>>,
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
    let perm = crate::authz::perm::TORRENT_MANAGE;
    crate::authz::require_perm(&state, &auth, perm).await?;
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
                "UPDATE torrents SET pos_state = $2, \
                 pos_state_until = $3, mtime = now() \
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
            // 白名单直接 400——旧版把拼错的 kind 静默挂成 free，管理端无感知。
            let kind = match body.promo_kind.as_deref() {
                None => "free",
                Some(k) if PROMO_KINDS.contains(&k) => k,
                Some(k) => {
                    return Err(DomainError::Validation(format!(
                        "未知促销类型：{k}（合法值：{PROMO_KINDS:?}）"
                    )));
                }
            };
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
                "INSERT INTO promotions (scope, torrent_id, \
                 kind, starts_at, ends_at, source, \
                 created_by) \
                 SELECT 'torrent', id, $2::promotion_kind_enum, \
                 now(), $3, 'manual', $4 \
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
            let who = (auth.id, auth.class_id as i16);
            for tid in &id_arr {
                crate::torrents::apply_torrent_tags(
                    db,
                    *tid,
                    &body.tag_ids,
                    who,
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
            // 乐观并发（F2 / FRAME §5）：挂/摘 H&R 改写整份 hr_policy，此前
            // last-write-wins——两版主并发，后者整条覆盖且无感知。携带
            // expected_mtime 时先 FOR UPDATE 比对，任一种子不匹配 → 409
            // （整批不动；非 RFC3339 也按冲突处理）。缺省不校验（灰度）。
            let expected = body.expected_mtime.clone().unwrap_or_default();
            if !expected.is_empty() {
                let rows: Vec<(i64, chrono::DateTime<chrono::Utc>)> =
                    sqlx::query_as(
                        "SELECT id, mtime FROM torrents WHERE id = ANY($1) \
                         FOR UPDATE",
                    )
                    .bind(&id_arr)
                    .fetch_all(db)
                    .await
                    .map_err(|e| DomainError::Internal(e.into()))?;
                let mut conflicts: Vec<String> = Vec::new();
                for (tid, cur) in rows {
                    let Some(got) = expected.get(&tid) else {
                        continue; // 键未携带版本戳：该种子不校验
                    };
                    let ok = chrono::DateTime::parse_from_rfc3339(got)
                        .map(|t| t.with_timezone(&chrono::Utc) == cur)
                        .unwrap_or(false);
                    if !ok {
                        conflicts.push(tid.to_string()); // FOR UPDATE 行 id 唯一
                    }
                }
                if !conflicts.is_empty() {
                    return Err(DomainError::VersionConflict(conflicts));
                }
            }
            // 深测 2026-10-03：挂 H&R 把站点缺省口径（hr_hours；days=14）落进
            // 种子 policy——只写 {"on":true} 会全靠 COALESCE，改设定不生效。
            let policy: serde_json::Value = if on {
                let hr_hours: i32 = sqlx::query_scalar(
                    "SELECT COALESCE(NULLIF((SELECT value FROM site_settings \
                     WHERE name = 'hr_hours'), '')::int, 48)",
                )
                .fetch_one(db)
                .await
                .unwrap_or(48)
                .clamp(1, 24 * 365);
                serde_json::json!({ "on": true, "seed_hours": hr_hours,
                    "days": 14 })
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
            // 四审 L3 P0：直改 medium/grade/edition 不反写 torrent_sections，
            // 详情页与筛选仍显旧值——维度一律走 change_sections 落列。
            if body.medium_id.is_some()
                || body.grade_id.is_some()
                || body.edition_id.is_some()
            {
                return Err(DomainError::Validation(
                    "媒介/学段/版本请用 change_sections 提交（旧三列不再可写）"
                        .into(),
                ));
            }
            // 全 COALESCE 旧写法在全 None 时仍命中 N 行回报「成功 N 条」，
            // 一个字段没改——要求显式给出目标分类。
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
            // 与单条编辑口同源（六类型 + 多值 + 字典归属），但用部分更新：
            // 整组重建会把没勾的维度全清掉；这里只动给定维度（一次一维）。
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
            // 给了但解析为空（如 {"dict_ids":[]}）= 清空该维度
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
                // 批量改维度可能清到派生维度（log_score/haslog），逐种子
                // 重算（幂等；与 manage.rs 编辑口同口径）
                crate::torrents::sync_log_dims(db, *tid).await?;
                n += 1;
            }
            n
        }
        // 状态机判定收 domain（FRAME §7 第 6 条）：delete 源态谓词现状为
        // 「无」（任意态可下架，含 3 幂等重放）——按矩阵生成合法源态集，
        // 判定并入同一条语句：UPDATE 仅命中合法源态行并 RETURNING 计数，
        // 少于批内 id 数即有非法态/不存在 → 整批 400 不半批静默（当前
        // 矩阵全放行，见 torrent_status 单测注记）。
        "delete" => {
            use crate::domain::torrent_status::legal_delist_sources;
            let ok_rows: Vec<i64> = sqlx::query(
                "UPDATE torrents SET approval_status = 3, mtime = now()                  WHERE id = ANY($1) AND approval_status = ANY($2)                  RETURNING id",
            )
            .bind(&id_arr)
            .bind(&legal_delist_sources())
            .fetch_all(db)
            .await
            .map_err(|e| DomainError::Internal(e.into()))?;
            let n = ok_rows.len() as u64;
            if n as usize != id_arr.len() {
                return Err(DomainError::Validation(
                    "批内存在迁移矩阵不允许下架（或不存在）的种子".into(),
                ));
            }
            n
        }
        _ => return Err(DomainError::Validation("未知批量动作".into())),
    };
    // 操作记录：每种动作写一条汇总（detail 带 ids），列表页可按种子过滤
    sqlx::query(
        "INSERT INTO torrent_operation_logs \
         (torrent_id, operator_id, action, detail) \
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
    // 批量动作改的是列表可见内容（分类/状态/统计/置顶位）：推进列表缓存代际
    if n > 0 {
        crate::torrent_http::bump_list_cache_gen(&state).await;
    }
    Ok(ok(serde_json::json!({ "affected": n as i64 })))
}
