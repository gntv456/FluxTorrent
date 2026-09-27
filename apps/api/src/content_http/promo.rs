//! M20 促销套餐与购买。从 content_http/misc.rs 按域拆出。
//! 0213：档位不再写死——读 promo_kinds / promo_kind_tiers 注册表，
//! 站方可自由增删档位（服务「不偏向任何 PT 类型」定位）。
//! 默认种子 = sticky1/sticky2/free，老站零行为变化。

use actix_web::{get, post, web, HttpRequest, HttpResponse};
use serde::Deserialize;

use crate::dto::ok;
use crate::economy_http::spend_spark_tx;
use crate::errors::{DomainError, DomainResult};
use crate::http::require_auth;
use crate::state::AppState;

/// 一个可购档位（kind + hours + price）
#[derive(sqlx::FromRow)]
struct TierRow {
    kind: String,
    hours: i32,
    price: i64,
}

/// 价目表（前端渲染档位；module_promo_buy=no 时返回 disabled）
/// 读 promo_kind_tiers ⋈ promo_kinds（enabled 双向过滤），按 sort_order/hours 排序。
#[get("/promo/plans")]
pub(super) async fn promo_plans(
    req: HttpRequest,
    state: web::Data<std::sync::Arc<AppState>>,
) -> DomainResult<HttpResponse> {
    let _auth = require_auth(&req, &state).await?;
    let enabled = promo_enabled(&state).await;
    let rows: Vec<TierRow> = sqlx::query_as(
        "SELECT t.kind, t.hours, t.price \
         FROM promo_kind_tiers t JOIN promo_kinds k ON k.kind = t.kind \
         WHERE k.enabled AND t.enabled \
         ORDER BY k.sort_order, k.kind, t.hours",
    )
    .fetch_all(&state.repo.db)
    .await
    .map_err(|e| DomainError::Internal(e.into()))?;
    // plans 保序：前端按 kind 分组渲染，顺序由本响应决定
    let mut plans = serde_json::Map::new();
    for r in rows {
        let entry = serde_json::json!({ "hours": r.hours, "price": r.price });
        plans
            .entry(r.kind)
            .or_insert_with(|| serde_json::Value::Array(Vec::new()))
            .as_array_mut()
            .expect("plans 值恒为数组")
            .push(entry);
    }
    // 档位显示元数据（前端不再硬编码 KIND_LABELS）
    let kinds: Vec<KindMetaRow> = sqlx::query_as(
        "SELECT kind, label_zh, label_en, effect, i18n_key FROM promo_kinds \
         WHERE enabled ORDER BY sort_order, kind",
    )
    .fetch_all(&state.repo.db)
    .await
    .map_err(|e| DomainError::Internal(e.into()))?;
    Ok(ok(serde_json::json!({
        "enabled": enabled,
        "plans": plans,
        "kinds": kinds,
    })))
}

/// 购买：所有登录用户可购（0173 放开，此前仅发布者/staff）；扣费（spark_ledger
/// 流水，幂等键含随机 nonce 由客户端提供——每次购买是独立消费行为，重试需带同一键）
/// → 效果按 promo_kinds.effect 分派：sticky* 写 pos_state/pos_state_until
/// （延长语义：在现有效期内续购则顺延），其余写 promotions（torrent 专属）。
/// 生效校验在写入端完成；列表 sticky_expr 与促销裁决天然消费这些字段（零新查询）。
#[post("/promo/buy")]
pub(super) async fn promo_buy(
    req: HttpRequest,
    state: web::Data<std::sync::Arc<AppState>>,
    body: web::Json<PromoBuyReq>,
) -> DomainResult<HttpResponse> {
    let auth = require_auth(&req, &state).await?;
    if !promo_enabled(&state).await {
        return Err(DomainError::Validation("本功能未开放".into()));
    }
    // 档位合法性改由注册表判定（替代写死的 kind/hours 白名单）
    let effect: Option<String> = sqlx::query_scalar(
        "SELECT k.effect FROM promo_kinds k \
         JOIN promo_kind_tiers t ON t.kind = k.kind \
         WHERE k.kind = $1 AND k.enabled AND t.enabled AND t.hours = $2",
    )
    .bind(&body.kind)
    .bind(body.hours)
    .fetch_optional(&state.repo.db)
    .await
    .map_err(|e| DomainError::Internal(e.into()))?;
    let Some(effect) = effect else {
        return Err(DomainError::Validation(
            "该档位不可购买（类型/时长不存在或未开放）".into(),
        ));
    };
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
    let price: i64 = sqlx::query_scalar(
        "SELECT t.price FROM promo_kind_tiers t \
         JOIN promo_kinds k ON k.kind = t.kind \
         WHERE t.kind = $1 AND t.hours = $2 AND k.enabled AND t.enabled",
    )
    .bind(&body.kind)
    .bind(body.hours)
    .fetch_optional(&state.repo.db)
    .await
    .map_err(|e| DomainError::Internal(e.into()))?
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
    match effect.as_str() {
        // 置顶：GREATEST(现有到期, 新到期) 顺延；级别由 effect 决定（sticky1/sticky2）
        "sticky1" | "sticky2" => {
            let level: i16 = if effect == "sticky1" { 1 } else { 2 };
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
        // 其余（free/x2/x2free/half/x2half/p30 等）：专属 torrent 促销。
        // 计费裁决在 worker（billing_multipliers 同口径），列表角标同数据源。
        other => {
            // effect 必须落在 promotions.kind 枚举内，否则 INSERT 报错——
            // 提前挡成 400（站长在后台把 effect 配错时报错信息要好懂）
            let valid: bool = sqlx::query_scalar(
                "SELECT EXISTS(SELECT 1 FROM unnest(\
                 enum_range(NULL::promotion_kind_enum)) AS e \
                 WHERE e::text = $1)",
            )
            .bind(other)
            .fetch_one(&mut *tx)
            .await
            .map_err(|e| DomainError::Internal(e.into()))?;
            if !valid {
                return Err(DomainError::Validation(
                    "档位效果配置无效（不在促销类型枚举内）".into(),
                ));
            }
            sqlx::query(
                "INSERT INTO promotions \
                 (scope, torrent_id, kind, starts_at, ends_at, source, created_by) \
                 VALUES ('torrent', $1, $2::promotion_kind_enum, now(), $3, \
                 'manual', $4)",
            )
            .bind(body.torrent_id)
            .bind(other)
            .bind(ends)
            .bind(auth.id)
            .execute(&mut *tx)
            .await
            .map_err(|e| DomainError::Internal(e.into()))?;
        }
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

/// 档位显示元数据（前端不再硬编码标签）
#[derive(sqlx::FromRow, serde::Serialize)]
pub(super) struct KindMetaRow {
    pub(super) kind: String,
    pub(super) label_zh: String,
    pub(super) label_en: String,
    pub(super) effect: String,
    pub(super) i18n_key: Option<String>,
}

/// 模块开关（module_promo_buy = no 时全套隐藏）
pub(super) async fn promo_enabled(
    state: &web::Data<std::sync::Arc<AppState>>,
) -> bool {
    sqlx::query_scalar::<_, String>(
        "SELECT value FROM site_settings WHERE name = 'module_promo_buy'",
    )
    .fetch_optional(&state.repo.db)
    .await
    .ok()
    .flatten()
    .map(|v| v != "no")
    .unwrap_or(true)
}

#[derive(Deserialize)]
struct PromoBuyReq {
    torrent_id: i64,
    /// 档位标识（promo_kinds.kind，如 sticky1/sticky2/free 或站方自定义）
    kind: String,
    /// 时长（小时；须与 promo_kind_tiers 某行匹配）
    hours: i32,
    #[serde(default)]
    idempotency_key: Option<String>,
}
