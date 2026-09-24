//! M20 促销套餐与购买。从 content_http/misc.rs 按域拆出。

use actix_web::{get, post, web, HttpRequest, HttpResponse};
use serde::Deserialize;

use crate::dto::ok;
use crate::economy_http::spend_spark_tx;
use crate::errors::{DomainError, DomainResult};
use crate::http::require_auth;
use crate::state::AppState;

/// 价目表（前端渲染档位；module_promo_buy=no 时返回 disabled）
#[get("/promo/plans")]
pub(super) async fn promo_plans(
    req: HttpRequest,
    state: web::Data<std::sync::Arc<AppState>>,
) -> DomainResult<HttpResponse> {
    let _auth = require_auth(&req, &state).await?;
    let enabled = sqlx::query_scalar::<_, String>(
        "SELECT value FROM site_settings WHERE name = 'module_promo_buy'",
    )
    .fetch_optional(&state.repo.db)
    .await
    .ok()
    .flatten()
    .map(|v| v != "no")
    .unwrap_or(true);
    let mut plans = serde_json::Map::new();
    for kind in ["sticky1", "sticky2", "free"] {
        let mut entries = Vec::new();
        for hours in [24i32, 72i32] {
            if let Some(price) = promo_price(&state.repo.db, kind, hours).await
            {
                entries.push(
                    serde_json::json!({ "hours": hours, "price": price }),
                );
            }
        }
        plans.insert(kind.to_string(), entries.into());
    }
    Ok(ok(
        serde_json::json!({ "enabled": enabled, "plans": plans }),
    ))
}

/// 购买：所有登录用户可购（0173 放开，此前仅发布者/staff）；扣费（spark_ledger
/// 流水，幂等键含随机 nonce 由客户端提供——每次购买是独立消费行为，重试需带同一键）
/// → 置顶写 pos_state/pos_state_until（延长语义：在现有效期内续购则顺延），
/// 免费写 promotions（torrent 专属 free）。生效校验在写入端完成；
/// 列表 sticky_expr 与促销裁决天然消费这些字段（零新查询）。
#[post("/promo/buy")]
pub(super) async fn promo_buy(
    req: HttpRequest,
    state: web::Data<std::sync::Arc<AppState>>,
    body: web::Json<PromoBuyReq>,
) -> DomainResult<HttpResponse> {
    let auth = require_auth(&req, &state).await?;
    if !["sticky1", "sticky2", "free"].contains(&body.kind.as_str()) {
        return Err(DomainError::Validation(
            "kind 需为 sticky1/sticky2/free".into(),
        ));
    }
    if ![24, 72].contains(&body.hours) {
        return Err(DomainError::Validation("hours 需为 24 或 72".into()));
    }
    let enabled = sqlx::query_scalar::<_, String>(
        "SELECT value FROM site_settings WHERE name = 'module_promo_buy'",
    )
    .fetch_optional(&state.repo.db)
    .await
    .ok()
    .flatten()
    .map(|v| v != "no")
    .unwrap_or(true);
    if !enabled {
        return Err(DomainError::Validation("本功能未开放".into()));
    }
    // 幂等键必须带 uid 前缀（P0）：裸客户端键跨用户碰撞时，B 的购买会被误判为
    // A 的重放而不扣款——效果照发（重放闸门在 spend 之后）或静默丢失，二选一都不对。
    let idem = body
        .idempotency_key
        .clone()
        .filter(|k| !k.trim().is_empty())
        .map(|k| format!("promo:{}:{}", auth.id, k.trim()))
        .ok_or(DomainError::Validation("缺少 idempotency_key".into()))?;

    // 0173 放开全员可购：查询仅作存在性校验（不存在/未过审 → 404），
    // 不再限制发布者/staff。
    let owner: Option<i64> = sqlx::query_scalar(
        "SELECT owner_id FROM torrents WHERE id = $1 AND approval_status = 1",
    )
    .bind(body.torrent_id)
    .fetch_optional(&state.repo.db)
    .await
    .map_err(|e| DomainError::Internal(e.into()))?;
    if owner.is_none() {
        return Err(DomainError::NotFound(body.torrent_id));
    }
    let price = promo_price(&state.repo.db, &body.kind, body.hours)
        .await
        .ok_or_else(|| {
            DomainError::Validation("该档位未定价，请联系站方".into())
        })?;

    let mut tx = state
        .repo
        .db
        .begin()
        .await
        .map_err(|e| DomainError::Internal(e.into()))?;
    // 幂等重放必须终止（P0）：重放时 spend 不扣款，若继续执行则置顶白续 24/72h、
    // free 白加一条促销 = 印钞口。对照 games scratch 的同款闸门。
    if !matches!(
        spend_spark_tx(
            &mut tx,
            auth.id,
            price,
            "promo_buy",
            &idem,
            "torrent",
            body.torrent_id,
        )
        .await?,
        crate::economy_http::SpendOutcome::Spent
    ) {
        return Err(DomainError::Validation(
            "该笔购买已受理，请勿重复提交".into(),
        ));
    }
    let ends: chrono::DateTime<chrono::Utc> =
        sqlx::query_scalar("SELECT now() + make_interval(hours => $1)")
            .bind(body.hours)
            .fetch_one(&mut *tx)
            .await
            .map_err(|e| DomainError::Internal(e.into()))?;
    match body.kind.as_str() {
        // 置顶：GREATEST(现有到期, 新到期) 顺延；级别取本次购买档
        "sticky1" | "sticky2" => {
            let level = if body.kind == "sticky1" { 1i16 } else { 2i16 };
            sqlx::query(
                "UPDATE torrents SET \
                    pos_state = $2, \
                    pos_state_until = GREATEST(COALESCE(pos_state_until, now()), now()) + make_interval(hours => $3) \
                 WHERE id = $1",
            )
            .bind(body.torrent_id)
            .bind(level)
            .bind(body.hours)
            .execute(&mut *tx)
            .await
            .map_err(|e| DomainError::Internal(e.into()))?;
        }
        // 限时免费：专属 torrent 促销（worker 计费裁决/列表角标同一数据源）
        "free" => {
            sqlx::query(
                "INSERT INTO promotions (scope, torrent_id, kind, starts_at, ends_at, source, created_by) \
                 VALUES ('torrent', $1, 'free', now(), $2, 'manual', $3)",
            )
            .bind(body.torrent_id)
            .bind(ends)
            .bind(auth.id)
            .execute(&mut *tx)
            .await
            .map_err(|e| DomainError::Internal(e.into()))?;
        }
        _ => unreachable!(),
    }
    sqlx::query(
        "INSERT INTO promo_purchases (user_id, torrent_id, kind, hours, price, idempotency_key, ends_at) \
         VALUES ($1, $2, $3, $4, $5, $6, $7) ON CONFLICT (idempotency_key) DO NOTHING",
    )
    .bind(auth.id)
    .bind(body.torrent_id)
    .bind(&body.kind)
    .bind(body.hours)
    .bind(price)
    .bind(&idem)
    .bind(ends)
    .execute(&mut *tx)
    .await
    .map_err(|e| DomainError::Internal(e.into()))?;
    tx.commit()
        .await
        .map_err(|e| DomainError::Internal(e.into()))?;
    state
        .repo
        .audit(Some(auth.id), "promo_buy", Some(body.torrent_id))
        .await;
    Ok(ok(serde_json::json!({
        "torrent_id": body.torrent_id, "kind": body.kind,
        "hours": body.hours, "price": price, "ends_at": ends,
    })))
}

/// 价目/开关读取：promo_price.{kind}.{hours}（缺省价见迁移 0101；未定价的档位不可购买）
pub(super) async fn promo_price(
    db: &sqlx::PgPool,
    kind: &str,
    hours: i32,
) -> Option<i64> {
    sqlx::query_scalar::<_, String>(
        "SELECT value FROM site_settings WHERE name = $1",
    )
    .bind(format!("promo_price.{kind}.{hours}"))
    .fetch_optional(db)
    .await
    .ok()
    .flatten()
    .and_then(|v| v.trim().parse::<i64>().ok())
    .filter(|&p| p > 0)
}

#[derive(Deserialize)]
struct PromoBuyReq {
    torrent_id: i64,
    /// sticky1 一级置顶 | sticky2 二级置顶 | free 限时免费
    kind: String,
    /// 24 | 72（小时；与价目键对齐）
    hours: i32,
    #[serde(default)]
    idempotency_key: Option<String>,
}
