//! 券的管理端「读 + 作废」（发放面二轮，从 user_revoke.rs 拆出守 300 行门禁）。
//!
//! 读与管放在同一处是有理由的：此前只有用户侧 `/me/vouchers`，后台**没有读口**，
//! 于是 `void` 即使修好也没法用——看不到某人名下有哪些券、哪张已核销、
//! 哪张是管理发放的，就没人能安全地决定「作废几张」。两个端点同闸 `prop.manage`。

use actix_web::{get, post, web, HttpRequest, HttpResponse};
use serde::Deserialize;

use crate::dto::ok;
use crate::errors::{DomainError, DomainResult};
use crate::state::AppState;

use super::guard::{ensure_outranks, staff};

/// 券的后台浏览（发放面二轮）：站长要能回答「这个号手上有什么券、哪张什么时候失效」。
///
/// 此前**完全没有后台读口**——只有用户侧 `/me/vouchers`。于是 `void` 端点修好了
/// 也没法用：看不到就不知道该作废几张、也分不清哪张是自己发的。
/// 权限与 `void`/回收同闸（`prop.manage`），读与管的门槛不能不一致。
#[derive(sqlx::FromRow, serde::Serialize)]
struct VoucherRow {
    id: i64,
    user_id: i64,
    username: String,
    kind: String,
    /// admin = 管理发放，shop = 用户自己花钱买的（作废只动前者）
    source: String,
    granted_at: chrono::DateTime<chrono::Utc>,
    expires_at: chrono::DateTime<chrono::Utc>,
    used_at: Option<chrono::DateTime<chrono::Utc>>,
    used_torrent_id: Option<i64>,
    /// 已过期（服务端算，别让界面再持一份 now 口径）
    expired: bool,
}

#[derive(serde::Deserialize)]
struct VoucherListQ {
    uid: i64,
    /// true = 只列未核销的（要作废时看的就是这一堆）
    #[serde(default)]
    unused_only: bool,
}

#[get("/admin/user-vouchers")]
pub(super) async fn admin_voucher_list(
    req: HttpRequest,
    state: web::Data<std::sync::Arc<AppState>>,
    q: web::Query<VoucherListQ>,
) -> DomainResult<HttpResponse> {
    let auth = staff(&req, &state).await?;
    crate::authz::require_perm(&state, &auth, crate::authz::perm::PROP_MANAGE)
        .await?;
    let rows: Vec<VoucherRow> = sqlx::query_as(
        r#"SELECT v.id, v.user_id, u.username, v.kind, v.source,
                  v.granted_at, v.expires_at, v.used_at, v.used_torrent_id,
                  (v.expires_at < now()) AS expired
           FROM user_vouchers v
           JOIN users u ON u.id = v.user_id
           WHERE v.user_id = $1
             AND (NOT $2 OR (v.used_at IS NULL
                             AND v.used_torrent_id IS NULL))
           ORDER BY v.id DESC LIMIT 200"#,
    )
    .bind(q.uid)
    .bind(q.unused_only)
    .fetch_all(&state.repo.db)
    .await
    .map_err(|e| DomainError::Internal(e.into()))?;
    Ok(ok(rows))
}

/// 券作废：只收**管理员发放**且未核销的券，整批按张数上限收回。
///
/// 0291 修掉的三条：
/// - 语句里只有 `$2` 没 `$1`，Postgres 在 prepare 期就失败，而调用处用 `?`
///   上抛 ⇒ 券已经删了、接口回 500、库存没退、审计也没落（实测 B2/B4/B5）；
/// - 没有 `source` 过滤 ⇒ 会把用户花魔力自购的未用券一起吞掉（实测 B3，
///   注释写着「管理员发放券」，SQL 却不认这一步）；
/// - 库存回滚按 SKU kind 归位：一张语句里用 CTE 把作废的券按 free/neutral
///   分组退回对应道具池，不新增请求字段。
#[derive(Deserialize)]
pub(super) struct VoucherVoidReq {
    user_id: i64,
    /// 作废张数上限（1-50）：防误操作把某人全部券清掉时至少要先想清楚数量
    limit: i64,
}

#[post("/admin/user-vouchers/void")]
pub(super) async fn admin_voucher_void(
    req: HttpRequest,
    state: web::Data<std::sync::Arc<AppState>>,
    body: web::Json<VoucherVoidReq>,
) -> DomainResult<HttpResponse> {
    let auth = staff(&req, &state).await?;
    crate::authz::require_perm(&state, &auth, crate::authz::perm::PROP_MANAGE)
        .await?;
    ensure_outranks(&state.repo.db, auth.class_id, body.user_id).await?;
    if !(1..=50).contains(&body.limit) {
        return Err(DomainError::Validation("limit 需在 1-50".into()));
    }
    // 单条语句里完成「挑 → 删 → 退配额 → 报数」，全程一个事务快照，
    // 不会出现「删了券但配额没退」的半截状态。
    let n: i64 = sqlx::query_scalar(
        r#"WITH picked AS (
               SELECT id FROM user_vouchers
               WHERE user_id = $1 AND source = 'admin'
                 AND used_at IS NULL AND used_torrent_id IS NULL
               ORDER BY id LIMIT $2),
           del AS (
               DELETE FROM user_vouchers
               WHERE id IN (SELECT id FROM picked)
               RETURNING kind),
           back AS (
               UPDATE shop_items si
               SET stock_used = GREATEST(0, si.stock_used - x.cnt)
               FROM (SELECT CASE WHEN kind = 'neutral'
                                 THEN 'voucher_neutral'
                                 ELSE 'voucher_free' END AS sku,
                            count(*)::bigint AS cnt
                     FROM del GROUP BY 1) x
               WHERE si.kind = x.sku AND si.active
                 AND si.stock_quota IS NOT NULL)
         SELECT count(*)::bigint FROM del"#,
    )
    .bind(body.user_id)
    .bind(body.limit)
    .fetch_one(&state.repo.db)
    .await
    .map_err(|e| DomainError::Internal(e.into()))?;
    if n == 0 {
        return Err(DomainError::NotFound(body.user_id));
    }
    state
        .repo
        .audit_detail(
            Some(auth.id),
            "voucher.void",
            Some(body.user_id),
            None,
            Some(serde_json::json!({ "voided": n, "source": "admin" })),
        )
        .await;
    Ok(ok(serde_json::json!({ "voided": n })))
}
