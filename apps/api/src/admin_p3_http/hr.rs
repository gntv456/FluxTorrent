//! P1-3 H&R 总览（参考站 user/hit-and-runs 口径）
//! 从 admin_p3_http.rs 按域拆出。

use actix_web::{get, post, web, HttpRequest, HttpResponse};
use serde::Deserialize;

use crate::dto::ok;
use crate::errors::{DomainError, DomainResult};
use crate::state::AppState;

use super::{check_ids, staff};

// ============ P1-3 H&R 总览（参考站 user/hit-and-runs 口径） ============

#[derive(sqlx::FromRow, serde::Serialize)]
struct HrRecordRow {
    user_id: i64,
    username: String,
    torrent_id: i64,
    torrent_name: Option<String>,
    /// H&R 快照按 (user_id, torrent_id) 存义务，流水从 snatches 现取；
    /// 复活/清理任务删掉 snatch 后这里就是 NULL——曾按 i64 解码导致
    /// 整个后台 H&R 页 500（一行缺流水，全表都读不出）。
    uploaded: Option<i64>,
    downloaded: Option<i64>,
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
    q: crate::http::Q<HrListQ>,
) -> DomainResult<HttpResponse> {
    let auth = staff(&req, &state).await?;
    crate::authz::require_perm(&state, &auth, crate::authz::perm::HR_VIEW)
        .await?;
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
    .bind(crate::dto::page_window(q.page, q.per_page).1)
    .bind(crate::dto::page_window(q.page, q.per_page).0)
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

/// 赦免一条 H&R 违规的**唯一判据**（批四十三 P-165~P-167；单发口、gaps 批量口、
/// 本模块批量口原先各写一份 SQL，三份在三天里漂了三处）：
///
/// ① **操作者须严格高于目标**。成员侧同一动作 `/me/hr/pardon` 要付站点设定
/// `cancel_hr` 的火花（实测本站 10000），而管理三口谁都能按 ⇒ 有 `hr.pardon`
/// 权的职员（class ≥ 90）可**零成本赦免自己**那条违规，也可赦免上级；
/// `ensure_outranks` 里 `actor <= target` 即拒，顺带把「赦自己」一起挡掉。
/// ② **只赦 `violated`**。`open` 是「还没到期」的进行中义务，worker 的判定线是
/// `WHERE status = 'open' AND deadline < now()`；把它抹成 pardoned 后这条线永远
/// 不再命中，义务凭空消失，而 `hr_violations` 里连「欠多少做种时长」的依据都没有
/// （实测 p3 批量口就是这么干的，单发口同一目标回 400）。
/// ③ **理由必须落库**。三口原先都强制 `note` 非空，然后一个都不存 —— 0285 建好的
/// `audit_detail` 槽位（`audit_log.ref->>'reason'`）就在旁边没用；现在进审计，
/// 并给被赦的人留一条 `user_modify_logs`（纠纷仲裁时看得见）。
///
/// 返回 `true` = 真赦免了一条；`false` = 没有待赦记录（调用方决定报 400 还是
/// 计入 skipped，响应形状不变，免得动前端）。`action` 由调用方给（单发/批量
/// 在台账里仍是两个动作名，`apps/web/i18n/audit-labels.ts` 已各自有标签）。
pub(crate) async fn pardon_one(
    state: &web::Data<std::sync::Arc<AppState>>,
    actor: &crate::http::AuthUser,
    action: &str,
    uid: i64,
    tid: i64,
    note: &str,
) -> DomainResult<bool> {
    crate::admin_http::guard::ensure_outranks(
        &state.repo.db,
        actor.class_id,
        uid,
    )
    .await?;
    let n = sqlx::query(
        "UPDATE hr_snapshots SET status = 'pardoned', pardoned_by = $1, \
         updated_at = now() WHERE user_id = $2 AND torrent_id = $3 \
         AND status = 'violated'",
    )
    .bind(actor.id)
    .bind(uid)
    .bind(tid)
    .execute(&state.repo.db)
    .await
    .map_err(|e| DomainError::Internal(e.into()))?
    .rows_affected();
    if n == 0 {
        return Ok(false);
    }
    // 展示层读 snatches.hr_flag，而 worker 只置 TRUE（全库无 FALSE 路径）
    // ⇒ 赦免后不回清，用户侧 H&R 角标会一直亮着
    sqlx::query(
        "UPDATE snatches SET hr_flag = FALSE WHERE user_id = $1 \
         AND torrent_id = $2",
    )
    .bind(uid)
    .bind(tid)
    .execute(&state.repo.db)
    .await
    .map_err(|e| DomainError::Internal(e.into()))?;
    sqlx::query(
        "UPDATE hr_violations SET resolved_at = now(), resolved_by = $1 \
         WHERE user_id = $2 AND torrent_id = $3 AND resolved_at IS NULL",
    )
    .bind(actor.id)
    .bind(uid)
    .bind(tid)
    .execute(&state.repo.db)
    .await
    .map_err(|e| DomainError::Internal(e.into()))?;
    state
        .repo
        .audit_detail(
            Some(actor.id),
            action,
            Some(tid),
            Some(note),
            Some(serde_json::json!({ "target_user": uid })),
        )
        .await;
    super::modify_log(
        &state.repo.db,
        uid,
        Some(actor.id),
        &format!("H&R 违规已赦免（种 {tid}）：{note}"),
    )
    .await;
    Ok(true)
}

/// 批量前的整批护栏预扫：任一目标不可赦免就**一个都不动**地拒掉，
/// 免得留下「赦了一半、另一半被权限挡下」的中间态（`pardon_one` 内部
/// 还会再判一次，同一份判据）。
pub(crate) async fn ensure_all_pardonable(
    state: &web::Data<std::sync::Arc<AppState>>,
    actor: &crate::http::AuthUser,
    ids: &[i64],
) -> DomainResult<()> {
    for uid in ids {
        crate::admin_http::guard::ensure_outranks(
            &state.repo.db,
            actor.class_id,
            *uid,
        )
        .await?;
    }
    Ok(())
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
    crate::authz::require_perm(&state, &auth, crate::authz::perm::HR_PARDON)
        .await?;
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
    // 判据收在 `pardon_one`（本文件上面那份）——此前这里自己写了一条
    // `status IN ('violated','open')` 的 SQL，比另两口多赦了「还没到期」的
    // 进行中义务，且理由不落库。整批先过一遍护栏，避免赦到一半被权限挡下。
    ensure_all_pardonable(
        &state,
        &auth,
        &body.records.iter().map(|r| r.user_id).collect::<Vec<_>>(),
    )
    .await?;
    let mut pardoned: i64 = 0;
    for r in &body.records {
        if pardon_one(
            &state,
            &auth,
            "hr.batch_pardon",
            r.user_id,
            r.torrent_id,
            &body.note,
        )
        .await?
        {
            pardoned += 1;
        }
    }
    Ok(ok(serde_json::json!({ "pardoned": pardoned })))
}
