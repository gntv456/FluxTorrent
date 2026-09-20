//! 经济系统 HTTP 接口（M11 商店/流水/银行 + M12 签到 + M13 站免池）。

use actix_web::{get, post, web, HttpRequest, HttpResponse, Responder};
use serde::Deserialize;
use sqlx::PgPool;
use uuid::Uuid;

use crate::dto::ok;
use crate::economy::{
    self, checkin_reward, early_penalty, loan_rate_bp, maturity_interest,
    term_rate, DEMAND_RATE_BP, LOAN_TERMS, VALID_TERMS,
};
use crate::errors::{DomainError, DomainResult};
use crate::http::require_auth;
use crate::state::AppState;

/// 银行参数（site_settings 可配，缺省与 0048 迁移一致）
struct BankSettings {
    min_deposit: i64,
    max_deposit: i64,
    min_demand: i64,
    loan_ratio: i64,
    loan_constant: i64,
    min_loan: i64,
    demand_rate_bp: i32,
    penalty_bp: i32,
    overdue_penalty_bp: i32,
    #[allow(dead_code)] // worker 读 site_settings，API 侧仅透传给前端不需要
    auto_deduct_days: i32,
    #[allow(dead_code)]
    allow_negative: bool,
}

async fn bank_settings(db: &PgPool) -> BankSettings {
    async fn get(db: &PgPool, name: &str) -> Option<String> {
        sqlx::query_scalar::<_, String>(
            "SELECT value FROM site_settings WHERE name = $1",
        )
        .bind(name)
        .fetch_optional(db)
        .await
        .ok()
        .flatten()
    }
    BankSettings {
        min_deposit: get(db, "bank_min_deposit")
            .await
            .and_then(|v| v.parse().ok())
            .unwrap_or(100),
        max_deposit: get(db, "bank_max_deposit")
            .await
            .and_then(|v| v.parse().ok())
            .unwrap_or(1_000_000),
        min_demand: get(db, "bank_min_demand")
            .await
            .and_then(|v| v.parse().ok())
            .unwrap_or(100),
        loan_ratio: get(db, "bank_loan_ratio")
            .await
            .and_then(|v| v.parse().ok())
            .unwrap_or(100),
        loan_constant: get(db, "bank_loan_ratio_constant")
            .await
            .and_then(|v| v.parse().ok())
            .unwrap_or(1000),
        min_loan: get(db, "bank_min_loan")
            .await
            .and_then(|v| v.parse().ok())
            .unwrap_or(100),
        demand_rate_bp: get(db, "bank_demand_rate_bp")
            .await
            .and_then(|v| v.parse().ok())
            .unwrap_or(DEMAND_RATE_BP),
        penalty_bp: get(db, "bank_penalty_rate_bp")
            .await
            .and_then(|v| v.parse().ok())
            .unwrap_or(50),
        overdue_penalty_bp: get(db, "bank_overdue_penalty_bp")
            .await
            .and_then(|v| v.parse().ok())
            .unwrap_or(50),
        auto_deduct_days: get(db, "bank_auto_deduct_days")
            .await
            .and_then(|v| v.parse().ok())
            .unwrap_or(7),
        allow_negative: get(db, "bank_allow_negative")
            .await
            .map(|v| v == "true")
            .unwrap_or(false),
    }
}

/// 经济路由（挂载进主 /api/v1 scope，单 scope 避免遮蔽）
pub fn mount_economy(scope: actix_web::Scope) -> actix_web::Scope {
    scope
        .service(shop_items)
        .service(shop_buy)
        .service(my_spark)
        .service(my_ledger)
        .service(bank_deposit)
        .service(bank_withdraw)
        .service(bank_list)
        .service(bank_overview)
        .service(demand_deposit)
        .service(demand_withdraw)
        .service(loan_apply)
        .service(loan_repay)
        .service(checkin)
        .service(checkin_status)
        .service(pool_status)
        .service(pool_donate)
        .service(dressup_list)
        .service(dressup_wear)
        .service(my_vouchers)
        .service(voucher_use)
        .service(spark_flow_report)
        .service(torznab_caps)
        .service(torznab_search)
        .service(fundings_list)
        .service(funding_create)
        .service(funding_contribute)
        .service(funding_my)
}

/// 动账核心：余额充足校验 + 负流水 + 余额快照更新（单事务）。
/// 幂等键唯一约束（shop_orders/应用层先查）防重复扣款。
/// 扣款结果：区分真实扣款与幂等重放（调用方据此决定是否执行副作用）。
/// must_use：丢弃该返回值继续执行副作用 = 幂等重放印钞口（审计 P0 整类缺陷的根因）。
#[must_use = "重放（Replayed）时不会再扣款，丢弃返回值继续执行副作用会造成资金凭空入账"]
pub enum SpendOutcome {
    Spent,
    Replayed,
}

/// spend_spark 的传入事务版本：调用方把扣款与其它写操作并入同一事务
/// （如求种悬赏冻结与建单原子化）。语义/锁序与 spend_spark 完全一致。
pub async fn spend_spark_tx(
    tx: &mut sqlx::Transaction<'_, sqlx::Postgres>,
    user_id: i64,
    amount: i64,
    kind: &str,
    idem: &str,
    ref_type: &str,
    ref_id: i64,
) -> DomainResult<SpendOutcome> {
    let balance: i64 = sqlx::query_scalar(
        "SELECT spark_balance FROM users WHERE id = $1 FOR UPDATE",
    )
    .bind(user_id)
    .fetch_one(&mut **tx)
    .await
    .map_err(|e| DomainError::Internal(e.into()))?;
    // 幂等检查查询失败必须报错（P2 审计）：unwrap_or(false) 会把「查询失败」当成
    // 「未消费」——跨请求重放可击穿幂等。fail-close：宁可 5xx 也不冒双扣风险。
    let exists: bool = sqlx::query_scalar(
        "SELECT EXISTS(SELECT 1 FROM spark_ledger WHERE idempotency_key = $1)",
    )
    .bind(idem)
    .fetch_one(&mut **tx)
    .await
    .map_err(|e| DomainError::Internal(e.into()))?;
    if exists {
        return Ok(SpendOutcome::Replayed);
    }
    if balance < amount {
        return Err(DomainError::InsufficientSpark);
    }
    sqlx::query(
        "INSERT INTO spark_ledger (id, user_id, amount, kind, ref_type, ref_id, idempotency_key, balance_after) \
         VALUES (nextval('spark_ledger_id_seq'), $1, $2, $3, $4, $5, $6, $7)",
    )
    .bind(user_id)
    .bind(-amount)
    .bind(kind)
    .bind(ref_type)
    .bind(ref_id)
    .bind(idem)
    .bind(balance - amount)
    .execute(&mut **tx)
    .await
    .map_err(|e| DomainError::Internal(e.into()))?;
    sqlx::query(
        "UPDATE users SET spark_balance = spark_balance - $2 WHERE id = $1",
    )
    .bind(user_id)
    .bind(amount)
    .execute(&mut **tx)
    .await
    .map_err(|e| DomainError::Internal(e.into()))?;
    Ok(SpendOutcome::Spent)
}

pub async fn spend_spark(
    db: &PgPool,
    user_id: i64,
    amount: i64,
    kind: &str,
    idem: &str,
    ref_type: &str,
    ref_id: i64,
) -> DomainResult<SpendOutcome> {
    let mut tx = db
        .begin()
        .await
        .map_err(|e| DomainError::Internal(e.into()))?;
    // 幂等检查必须在余额检查之前：已成功扣过的键在余额不足时也应返回重放，
    // 而不是误报「余额不足」（P0：防并发双扣的锁序不变，行锁仍先取）
    let balance: i64 = sqlx::query_scalar(
        "SELECT spark_balance FROM users WHERE id = $1 FOR UPDATE",
    )
    .bind(user_id)
    .fetch_one(&mut *tx)
    .await
    .map_err(|e| DomainError::Internal(e.into()))?;
    // 幂等检查查询失败必须报错（P2 审计，同 tx 版口径）
    let exists: bool = sqlx::query_scalar(
        "SELECT EXISTS(SELECT 1 FROM spark_ledger WHERE idempotency_key = $1)",
    )
    .bind(idem)
    .fetch_one(&mut *tx)
    .await
    .map_err(|e| DomainError::Internal(e.into()))?;
    if exists {
        return Ok(SpendOutcome::Replayed);
    }
    if balance < amount {
        return Err(DomainError::InsufficientSpark);
    }
    sqlx::query(
        "INSERT INTO spark_ledger (id, user_id, amount, kind, ref_type, ref_id, idempotency_key, balance_after) \
         VALUES (nextval('spark_ledger_id_seq'), $1, $2, $3, $4, $5, $6, $7)",
    )
    .bind(user_id)
    .bind(-amount)
    .bind(kind)
    .bind(ref_type)
    .bind(ref_id)
    .bind(idem)
    .bind(balance - amount)
    .execute(&mut *tx)
    .await
    .map_err(|e| DomainError::Internal(e.into()))?;
    sqlx::query(
        "UPDATE users SET spark_balance = spark_balance - $2 WHERE id = $1",
    )
    .bind(user_id)
    .bind(amount)
    .execute(&mut *tx)
    .await
    .map_err(|e| DomainError::Internal(e.into()))?;
    tx.commit()
        .await
        .map_err(|e| DomainError::Internal(e.into()))?;
    Ok(SpendOutcome::Spent)
}

/// earn_spark 的传入事务版本：调用方把入账与其它写操作并入同一事务（签到/支取/收获）。
/// 语义/锁序与 earn_spark 完全一致；返回 SpendOutcome 复用「Spent=首次入账 /
/// Replayed=幂等键已存在」口径，调用方据 Replayed 处理撕裂残留。
pub async fn earn_spark_tx(
    tx: &mut sqlx::Transaction<'_, sqlx::Postgres>,
    user_id: i64,
    amount: i64,
    kind: &str,
    idem: &str,
) -> DomainResult<SpendOutcome> {
    let balance: i64 = sqlx::query_scalar(
        "SELECT spark_balance FROM users WHERE id = $1 FOR UPDATE",
    )
    .bind(user_id)
    .fetch_one(&mut **tx)
    .await
    .map_err(|e| DomainError::Internal(e.into()))?;
    // 幂等检查必须在行锁之后（P0：防并发双入账）
    // 幂等检查查询失败必须报错（P2 审计）：unwrap_or(false) 会把「查询失败」当成
    // 「未消费」——跨请求重放可击穿幂等。fail-close：宁可 5xx 也不冒双扣风险。
    let exists: bool = sqlx::query_scalar(
        "SELECT EXISTS(SELECT 1 FROM spark_ledger WHERE idempotency_key = $1)",
    )
    .bind(idem)
    .fetch_one(&mut **tx)
    .await
    .map_err(|e| DomainError::Internal(e.into()))?;
    if exists {
        return Ok(SpendOutcome::Replayed);
    }
    sqlx::query(
        "INSERT INTO spark_ledger (id, user_id, amount, kind, idempotency_key, balance_after) \
         VALUES (nextval('spark_ledger_id_seq'), $1, $2, $3, $4, $5)",
    )
    .bind(user_id)
    .bind(amount)
    .bind(kind)
    .bind(idem)
    .bind(balance + amount)
    .execute(&mut **tx)
    .await
    .map_err(|e| DomainError::Internal(e.into()))?;
    sqlx::query(
        "UPDATE users SET spark_balance = spark_balance + $2 WHERE id = $1",
    )
    .bind(user_id)
    .bind(amount)
    .execute(&mut **tx)
    .await
    .map_err(|e| DomainError::Internal(e.into()))?;
    Ok(SpendOutcome::Spent)
}

/// 入账（签到/利息/奖励）
pub async fn earn_spark(
    db: &PgPool,
    user_id: i64,
    amount: i64,
    kind: &str,
    idem: &str,
) -> DomainResult<()> {
    let mut tx = db
        .begin()
        .await
        .map_err(|e| DomainError::Internal(e.into()))?;
    let balance: i64 = sqlx::query_scalar(
        "SELECT spark_balance FROM users WHERE id = $1 FOR UPDATE",
    )
    .bind(user_id)
    .fetch_one(&mut *tx)
    .await
    .map_err(|e| DomainError::Internal(e.into()))?;
    // 幂等检查必须在行锁之后（P0：防并发双入账）
    // 幂等检查查询失败必须报错（P2 审计，同 tx 版口径）
    let exists: bool = sqlx::query_scalar(
        "SELECT EXISTS(SELECT 1 FROM spark_ledger WHERE idempotency_key = $1)",
    )
    .bind(idem)
    .fetch_one(&mut *tx)
    .await
    .map_err(|e| DomainError::Internal(e.into()))?;
    if exists {
        return Ok(());
    }
    sqlx::query(
        "INSERT INTO spark_ledger (id, user_id, amount, kind, idempotency_key, balance_after) \
         VALUES (nextval('spark_ledger_id_seq'), $1, $2, $3, $4, $5)",
    )
    .bind(user_id)
    .bind(amount)
    .bind(kind)
    .bind(idem)
    .bind(balance + amount)
    .execute(&mut *tx)
    .await
    .map_err(|e| DomainError::Internal(e.into()))?;
    sqlx::query(
        "UPDATE users SET spark_balance = spark_balance + $2 WHERE id = $1",
    )
    .bind(user_id)
    .bind(amount)
    .execute(&mut *tx)
    .await
    .map_err(|e| DomainError::Internal(e.into()))?;
    tx.commit()
        .await
        .map_err(|e| DomainError::Internal(e.into()))?;
    Ok(())
}

// ============ 商店（M11） ============

#[derive(sqlx::FromRow, serde::Serialize)]
struct ShopItem {
    id: i64,
    name: String,
    kind: String,
    price: i64,
}

#[get("/shop/items")]
async fn shop_items(
    state: web::Data<std::sync::Arc<AppState>>,
) -> DomainResult<impl Responder> {
    let items = sqlx::query_as::<_, ShopItem>(
        "SELECT id, name, kind, price FROM shop_items WHERE active = true ORDER BY price",
    )
    .fetch_all(&state.repo.db)
    .await
    .map_err(|e| DomainError::Internal(e.into()))?;
    Ok(ok(items))
}

#[derive(Deserialize)]
struct BuyReq {
    item_id: i64,
    #[serde(default)]
    idempotency_key: Option<String>,
}

#[post("/shop/buy")]
async fn shop_buy(
    req: HttpRequest,
    state: web::Data<std::sync::Arc<AppState>>,
    body: web::Json<BuyReq>,
) -> DomainResult<HttpResponse> {
    let auth = require_auth(&req, &state).await?;
    let item: Option<(String, String, i64, serde_json::Value)> = sqlx::query_as(
        "SELECT name, kind, price, config FROM shop_items WHERE id = $1 AND active = true",
    )
    .bind(body.item_id)
    .fetch_optional(&state.repo.db)
    .await
    .map_err(|e| DomainError::Internal(e.into()))?;
    let Some((name, kind, price, config)) = item else {
        return Err(DomainError::NotFound(body.item_id));
    };

    // 审计修复（P1 花钱买空气）：唯一性道具（装扮/头衔类）重复购买此前照扣全价、
    // 效果 ON CONFLICT DO NOTHING —— 已拥有者再买 = 花钱买空气。可叠加类
    //（邀请/券/上传量/VIP 时长）不在此列。拥有判定与 apply_item_effect 同表。
    if matches!(
        kind.as_str(),
        "avatar_frame" | "animated_avatar" | "rainbow_id" | "rainbow_name"
    ) {
        let item_id = config.get("item_id").and_then(|v| v.as_i64());
        if let Some(iid) = item_id {
            let owned: bool = sqlx::query_scalar(
                "SELECT EXISTS(SELECT 1 FROM user_dressups WHERE user_id = $1 AND item_id = $2)",
            )
            .bind(auth.id)
            .bind(iid)
            .fetch_one(&state.repo.db)
            .await
            .unwrap_or(false);
            if owned {
                return Err(DomainError::Validation(
                    "你已拥有该装扮，无需重复购买（装扮类道具不叠加）".into(),
                ));
            }
        }
    }

    // 幂等键必填（P1）：网络层重试必须携带同一键，否则双扣款。
    // 服务端键必须带 uid 前缀：裸客户端键跨用户碰撞时，B 的消费会被误判为 A 的重放。
    let idem = body
        .idempotency_key
        .clone()
        .filter(|k| !k.is_empty())
        .map(|k| format!("shop:{}:{}", auth.id, k.trim()))
        .ok_or(DomainError::Validation("缺少 idempotency_key".into()))?;
    let outcome = spend_spark(
        &state.repo.db,
        auth.id,
        price,
        "shop",
        &idem,
        "shop_item",
        body.item_id,
    )
    .await?;

    // 订单落库（幂等键唯一）
    sqlx::query(
        "INSERT INTO shop_orders (user_id, item_id, price, idempotency_key, config_snapshot) \
         VALUES ($1, $2, $3, $4, $5) ON CONFLICT (idempotency_key) DO NOTHING",
    )
    .bind(auth.id)
    .bind(body.item_id)
    .bind(price)
    .bind(&idem)
    .bind(config.clone())
    .execute(&state.repo.db)
    .await
    .map_err(|e| DomainError::Internal(e.into()))?;

    // 效果执行（CAS 置位，0085）：首次成功扣款 OR 重试补发（此前 Replayed 直接跳过
    // 效果分支——扣款成功但效果失败后重试 = 花钱买空气）。置位失败 = 效果已发过，跳过。
    let _ = outcome;
    let should_apply: Option<i64> = sqlx::query_scalar(
        "UPDATE shop_orders SET effect_applied = TRUE          WHERE user_id = $1 AND idempotency_key = $2 AND NOT effect_applied RETURNING id",
    )
    .bind(auth.id)
    .bind(&idem)
    .fetch_optional(&state.repo.db)
    .await
    .map_err(|e| DomainError::Internal(e.into()))?;
    if should_apply.is_some() {
        let mut cfg = config.clone();
        cfg["item_id"] = serde_json::json!(body.item_id);
        apply_item_effect(&state.repo.db, auth.id, &kind, &cfg).await?;
    }

    state
        .repo
        .audit(Some(auth.id), "shop_buy", Some(body.item_id))
        .await;
    Ok(ok(
        serde_json::json!({ "item": name, "price": price, "idempotency_key": idem }),
    ))
}

async fn apply_item_effect(
    db: &PgPool,
    user_id: i64,
    kind: &str,
    config: &serde_json::Value,
) -> DomainResult<()> {
    match kind {
        // 上传量：等值正流量流水（§6.2 快照刷新由 worker 聚合，这里直接加账）
        "upload_credit" => {
            let gb = config.get("gb").and_then(|v| v.as_i64()).unwrap_or(0);
            sqlx::query(
                "INSERT INTO traffic_ledger (id, user_id, torrent_id, delta_up, delta_down, window_start) \
                 VALUES (nextval('traffic_ledger_id_seq'), $1, 0, $2, 0, now())",
            )
            .bind(user_id)
            .bind(gb * 1024 * 1024 * 1024)
            .execute(db)
            .await
            .map_err(|e| DomainError::Internal(e.into()))?;
            sqlx::query(
                "UPDATE users SET uploaded = uploaded + $2 WHERE id = $1",
            )
            .bind(user_id)
            .bind(gb * 1024 * 1024 * 1024)
            .execute(db)
            .await
            .map_err(|e| DomainError::Internal(e.into()))?;
        }
        // 装扮（M25）：写入拥有记录（佩戴需显式调 /dressup/wear）
        "avatar_frame" | "animated_avatar" | "rainbow_id" | "rainbow_name" => {
            let item_id =
                config.get("item_id").and_then(|v| v.as_i64()).unwrap_or(0);
            sqlx::query(
                "INSERT INTO user_dressups (user_id, item_id, source) VALUES ($1, $2, 'buy')                  ON CONFLICT (user_id, item_id) DO NOTHING",
            )
            .bind(user_id)
            .bind(item_id)
            .execute(db)
            .await
            .map_err(|e| DomainError::Internal(e.into()))?;
        }
        // 邀请：直接发一枚 72h 有效邀请码
        "invite" => {
            let code = crate::domain::new_invite_code();
            sqlx::query("INSERT INTO invites (inviter_id, code, expires_at) VALUES ($1, $2, now() + interval '72 hours')")
                .bind(user_id)
                .bind(&code)
                .execute(db)
                .await
                .map_err(|e| DomainError::Internal(e.into()))?;
        }
        // 免费券/中性券（0073，Gazelle FL token 口径）：买入库为库存，使用走 /me/vouchers/use
        "voucher_free" | "voucher_neutral" => {
            let kind = config
                .get("kind")
                .and_then(|v| v.as_str())
                .unwrap_or("free");
            sqlx::query(
                "INSERT INTO user_vouchers (user_id, kind, source) VALUES ($1, $2, 'shop')",
            )
            .bind(user_id)
            .bind(kind)
            .execute(db)
            .await
            .map_err(|e| DomainError::Internal(e.into()))?;
        }
        // VIP 待遇到期延展（0079 G18：购买日 ≥ 到期日则从今天起算，否则续期——断购不惩罚）
        "vip" | "app_vip" => {
            let days =
                config.get("days").and_then(|v| v.as_i64()).unwrap_or(30);
            sqlx::query(
                "UPDATE users SET \
                    vip_until = GREATEST(COALESCE(vip_until, now()), now()) + make_interval(days => $2), \
                    donor = TRUE \
                 WHERE id = $1",
            )
            .bind(user_id)
            .bind(days)
            .execute(db)
            .await
            .map_err(|e| DomainError::Internal(e.into()))?;
        }
        // 免广告（donor 待遇；NP 口径：15 天档）
        "ad_free" => {
            let days =
                config.get("days").and_then(|v| v.as_i64()).unwrap_or(15);
            sqlx::query(
                "UPDATE users SET donor_until = GREATEST(COALESCE(donor_until, now()), now()) + make_interval(days => $2) \
                 WHERE id = $1",
            )
            .bind(user_id)
            .bind(days)
            .execute(db)
            .await
            .map_err(|e| DomainError::Internal(e.into()))?;
        }
        // 自定义头衔（NP 5000 魔力口径）：config.title 由商店 SKU 预置；用户可后续在 UserCP 改（同价）
        "custom_title" => {
            if let Some(t) = config.get("title").and_then(|v| v.as_str()) {
                if !t.trim().is_empty() && t.chars().count() <= 30 {
                    sqlx::query("UPDATE users SET title = $2 WHERE id = $1")
                        .bind(user_id)
                        .bind(t.trim())
                        .execute(db)
                        .await
                        .map_err(|e| DomainError::Internal(e.into()))?;
                }
            }
        }
        // 审计修复（P1 花钱买空气）：下列 SKU 此前落入 `_ => {}` 兜底，扣款后无任何效果。
        // gift_spark：等值火花立即入账（earn 幂等键绑订单号语义 shop:{uid}:{item}）
        "gift_spark" => {
            let sparks = config
                .get("spark")
                .or_else(|| config.get("sparks"))
                .and_then(|v| v.as_i64())
                .unwrap_or(0);
            if sparks > 0 {
                let item_id =
                    config.get("item_id").and_then(|v| v.as_i64()).unwrap_or(0);
                let idem = format!(
                    "shop-gift:{user_id}:{item_id}:{}",
                    chrono::Utc::now().timestamp()
                );
                earn_spark(db, user_id, sparks, "shop", &idem).await?;
            }
        }
        // charity：捐赠入 magic_pool + pool_donations（去向可查，v_pool_honor 可见）。
        // 金额取 config.spark，缺省按 SKU 价格全额入池（购买即捐赠语义）。
        "charity" => {
            let amount = config
                .get("spark")
                .or_else(|| config.get("sparks"))
                .and_then(|v| v.as_i64())
                .or_else(|| config.get("price").and_then(|v| v.as_i64()))
                .unwrap_or(0);
            if amount > 0 {
                let month = economy::pool_month(chrono::Utc::now());
                sqlx::query(
                    "INSERT INTO pool_donations (user_id, amount, month) VALUES ($1, $2, $3)",
                )
                .bind(user_id)
                .bind(amount)
                .bind(&month)
                .execute(db)
                .await
                .map_err(|e| DomainError::Internal(e.into()))?;
                sqlx::query(
                    "INSERT INTO magic_pool (month, donated_total) VALUES ($1, $2) \
                     ON CONFLICT (month) DO UPDATE SET donated_total = magic_pool.donated_total + $2",
                )
                .bind(&month)
                .bind(amount)
                .execute(db)
                .await
                .map_err(|e| DomainError::Internal(e.into()))?;
            }
        }
        // rename_card：入库存（生效走 UserCP 改名消费，见 username_change_logs）
        "rename_card" | "temp_invite" => {
            sqlx::query(
                "INSERT INTO user_vouchers (user_id, kind, source) VALUES ($1, $2, 'shop')",
            )
            .bind(user_id)
            .bind(kind)
            .execute(db)
            .await
            .map_err(|e| DomainError::Internal(e.into()))?;
        }
        _ => {} // 其余类型：权益标记后续按需扩展（佩戴/生效周期）
    }
    Ok(())
}

// ============ 免费券/中性券（0073，Gazelle FL token 口径） ============

#[derive(sqlx::FromRow, serde::Serialize)]
struct VoucherRow {
    id: i64,
    kind: String,
    source: String,
    granted_at: chrono::DateTime<chrono::Utc>,
    expires_at: chrono::DateTime<chrono::Utc>,
    used_torrent_id: Option<i64>,
    used_at: Option<chrono::DateTime<chrono::Utc>>,
}

/// 我的券库存（含已用/过期历史，前端按状态分组）
#[get("/me/vouchers")]
async fn my_vouchers(
    req: HttpRequest,
    state: web::Data<std::sync::Arc<AppState>>,
) -> DomainResult<HttpResponse> {
    let auth = require_auth(&req, &state).await?;
    let rows: Vec<VoucherRow> = sqlx::query_as(
        "SELECT id, kind, source, granted_at, expires_at, used_torrent_id, used_at \
         FROM user_vouchers WHERE user_id = $1 ORDER BY id DESC LIMIT 200",
    )
    .bind(auth.id)
    .fetch_all(&state.repo.db)
    .await
    .map_err(|e| DomainError::Internal(e.into()))?;
    Ok(ok(rows))
}

#[derive(Deserialize)]
struct VoucherUseReq {
    voucher_id: i64,
    torrent_id: i64,
}

/// 用券：把一张未用未过期的券绑定到种子（CAS 防并发双用）。
/// 生效在 worker 计费侧：该种该用户的下载增量按 0 计（free）/上下行均 0 计（neutral），
/// 当累计下载超过种子大小 4% 时核销（Gazelle slop 口径——防买了券只下 1% 就转移给别人用）。
#[post("/me/vouchers/use")]
async fn voucher_use(
    req: HttpRequest,
    state: web::Data<std::sync::Arc<AppState>>,
    body: web::Json<VoucherUseReq>,
) -> DomainResult<HttpResponse> {
    let auth = require_auth(&req, &state).await?;
    // 绑定即生效但不置 used_at：worker 计费侧以「used_torrent_id 已绑定 + used_at IS NULL」
    // 判定生效中的券，下载量过阈值后由核销语句置 used_at。旧版绑定时就写 used_at，
    // 导致券永远不被计费侧匹配（用户花钱买的权益确定性为 0，真实资损）。
    let n = sqlx::query(
        "UPDATE user_vouchers SET used_torrent_id = $3 \
         WHERE id = $1 AND user_id = $2 AND used_torrent_id IS NULL AND used_at IS NULL AND expires_at > now()",
    )
    .bind(body.voucher_id)
    .bind(auth.id)
    .bind(body.torrent_id)
    .execute(&state.repo.db)
    .await
    .map_err(|e| DomainError::Internal(e.into()))?
    .rows_affected();
    if n == 0 {
        return Err(DomainError::Validation("券不存在、已使用或已过期".into()));
    }
    state
        .repo
        .audit(Some(auth.id), "voucher.use", Some(body.torrent_id))
        .await;
    Ok(ok(serde_json::json!({
        "voucher_id": body.voucher_id,
        "torrent_id": body.torrent_id,
        "note": "已对该种子生效；下载量超过种子大小 4% 后自动核销"
    })))
}

// ============ 我的火花（M11） ============

#[get("/me/spark")]
async fn my_spark(
    req: HttpRequest,
    state: web::Data<std::sync::Arc<AppState>>,
) -> DomainResult<impl Responder> {
    let auth = require_auth(&req, &state).await?;
    let balance: i64 =
        sqlx::query_scalar("SELECT spark_balance FROM users WHERE id = $1")
            .bind(auth.id)
            .fetch_one(&state.repo.db)
            .await
            .unwrap_or(0);
    // 收益因子说明（设计稿：1.03x/5x/0.1x 可点击展开）—— 由做种状态推导
    let seeding_count: i64 = sqlx::query_scalar(
        "SELECT count(*) FROM snatches WHERE user_id = $1 AND seeding",
    )
    .bind(auth.id)
    .fetch_one(&state.repo.db)
    .await
    .unwrap_or(0);
    // 与 worker 同口径的僵尸阈值：max(2h, 2×announce_interval)（详见 worker 的 stale_peer_threshold_secs）
    let announce: i64 = sqlx::query_scalar::<_, String>(
        "SELECT value FROM site_settings WHERE name = 'announce_interval'",
    )
    .fetch_optional(&state.repo.db)
    .await
    .ok()
    .flatten()
    .and_then(|v| v.parse::<i64>().ok())
    .map(|v| v.clamp(60, 86400))
    .unwrap_or(1800);
    let stale_secs = (announce * 2).max(7200);
    // 收益构成：按规则名分组（种子档位 + 做种时长档位两类），与 worker 结算同源。
    // 0133 后新增「做种时长」维度 —— 玩家能看到"我为什么拿这么多"，这是相对 NP 的体验优势。
    let rules: Vec<(String, i64)> = sqlx::query_as(
        r#"
        SELECT rule, count(*) FROM (
            SELECT CASE
                     WHEN t.seeders <= 1 AND t.times_completed >= 3 THEN '濒危保种'
                     WHEN now() - t.created_at > interval '365 days' THEN '高龄种'
                     WHEN now() - t.created_at > interval '180 days' THEN '老种'
                     WHEN t.size >= 107374182400 THEN '大体积种'
                     WHEN t.size >= 26843545600 THEN '中体积种'
                     ELSE '日常种'
                   END AS rule
            FROM snatches s JOIN torrents t ON t.id = s.torrent_id
            WHERE s.user_id = $1 AND s.seeding
              AND NOT (s.connectable = 0 AND s.uploaded = 0)
              AND s.last_seen_at > now() - ($2::bigint * interval '1 second')
            UNION ALL
            SELECT CASE
                     WHEN s.seeded_seconds >= 31536000 THEN '传奇做种者（≥1年）'
                     WHEN s.seeded_seconds >= 15552000 THEN '资深做种者（6-12月）'
                     WHEN s.seeded_seconds >= 7776000 THEN '稳定做种者（3-6月）'
                     WHEN s.seeded_seconds >= 2592000 THEN '新晋做种者（1-3月）'
                     ELSE '新做种（<1月）'
                   END AS rule
            FROM snatches s
            WHERE s.user_id = $1 AND s.seeding
              AND NOT (s.connectable = 0 AND s.uploaded = 0)
              AND s.last_seen_at > now() - ($2::bigint * interval '1 second')
        ) x GROUP BY rule ORDER BY count(*) DESC
        "#,
    )
    .bind(auth.id)
    .bind(stale_secs)
    .fetch_all(&state.repo.db)
    .await
    .unwrap_or_default();
    // 真实预估（与 worker 结算同一套 DB 函数 + 同一套过滤，迁移 0133）：
    // 不再是 "10 + 颗数×2" 的粗估。
    let hourly_estimate: i64 = sqlx::query_scalar(
        r#"
        WITH p AS MATERIALIZED (SELECT * FROM seeding_params()),
        s AS (
            SELECT sum(seeding_torrent_bonus(
                       t.size, t.seeders,
                       (EXTRACT(EPOCH FROM (now() - t.created_at)) / 86400.0)::double precision,
                       t.times_completed,
                       (GREATEST(sn.seeded_seconds, 0) / 3600.0)::double precision,
                       p.vol_base, p.rarity_k, p.rarity_exp, p.scale)) AS bonus_raw,
                   bool_or(u.donor) AS donor
            FROM snatches sn
            JOIN torrents t ON t.id = sn.torrent_id
            JOIN users u ON u.id = sn.user_id
            CROSS JOIN p
            WHERE sn.user_id = $1 AND sn.seeding
              AND NOT (sn.connectable = 0 AND sn.uploaded = 0)
              AND sn.last_seen_at > now() - ($2::bigint * interval '1 second')
        )
        SELECT seeding_hourly(COALESCE(bonus_raw, 0), p.base, p.cap, p.curve_k, p.donor_mult,
                              COALESCE(donor, FALSE))
        FROM s CROSS JOIN p
        "#,
    )
    .bind(auth.id)
    .bind(stale_secs)
    .fetch_one(&state.repo.db)
    .await
    .unwrap_or(0);
    Ok(ok(serde_json::json!({
        "balance": balance,
        "seeding_count": seeding_count,
        "hourly_estimate": hourly_estimate,
        "reward_rules": rules,
        "formula_note": "每小时 = 底薪 + (2/π)·封顶·atan(Σ加成 × 曲线系数)，捐赠者整笔翻倍。每颗种子加成 =（种子档位分 + 做种时长档位分）× 体积因子 × 稀有度加成 × 标定系数；种子档位：濒危保种 2.0 / 高龄种 1.5 / 老种 1.0 / 大体积 0.75 / 中体积 0.5 / 日常 0.25；做种时长档位：≥1年 +2.0 / 6-12月 +1.0 / 3-6月 +0.75 / 1-3月 +0.5；体积按对数饱和（≥50GB 记满分，1GB 约 0.18）；稀有度独苗 ×1.6、人多趋近 ×1.0。",
    })))
}

#[derive(sqlx::FromRow, serde::Serialize)]
struct LedgerRow {
    amount: i64,
    kind: String,
    balance_after: Option<i64>,
    created_at: chrono::DateTime<chrono::Utc>,
}

#[derive(Deserialize)]
struct LedgerQuery {
    limit: Option<i64>,
}

#[get("/me/spark/ledger")]
async fn my_ledger(
    req: HttpRequest,
    state: web::Data<std::sync::Arc<AppState>>,
    q: web::Query<LedgerQuery>,
) -> DomainResult<impl Responder> {
    let auth = require_auth(&req, &state).await?;
    let rows = sqlx::query_as::<_, LedgerRow>(
        "SELECT amount, kind, balance_after, created_at FROM spark_ledger \
         WHERE user_id = $1 ORDER BY id DESC LIMIT $2",
    )
    .bind(auth.id)
    .bind(q.limit.unwrap_or(20).clamp(1, 50))
    .fetch_all(&state.repo.db)
    .await
    .map_err(|e| DomainError::Internal(e.into()))?;
    Ok(ok(rows))
}

// ============ 银行（M11） ============

#[derive(Deserialize)]
struct DepositReq {
    amount: i64,
    term_days: i32,
    /// 客户端幂等键（审计修复 P1：服务端随机键导致网络重试/双击=双扣款。
    /// 与 shop/funding 同口径：重试必须携带同一键；缺省回退随机键保持兼容）
    #[serde(default)]
    idempotency_key: Option<String>,
}

#[post("/bank/deposit")]
async fn bank_deposit(
    req: HttpRequest,
    state: web::Data<std::sync::Arc<AppState>>,
    body: web::Json<DepositReq>,
) -> DomainResult<HttpResponse> {
    let auth = require_auth(&req, &state).await?;
    if !VALID_TERMS.contains(&body.term_days) {
        return Err(DomainError::Validation(
            "期限仅支持 7/30/90/180/365 天".into(),
        ));
    }
    if body.amount <= 0 {
        return Err(DomainError::Validation("存款金额必须为正".into()));
    }
    let bs = bank_settings(&state.repo.db).await;
    if body.amount < bs.min_deposit {
        return Err(DomainError::Validation(format!(
            "定期存款单笔不少于 {} 魔力",
            bs.min_deposit
        )));
    }
    if bs.max_deposit > 0 && body.amount > bs.max_deposit {
        return Err(DomainError::Validation(format!(
            "定期存款单笔不可超过 {} 魔力",
            bs.max_deposit
        )));
    }
    let idem = body
        .idempotency_key
        .clone()
        .filter(|k| !k.trim().is_empty())
        .map(|k| format!("deposit:{}:{}", auth.id, k.trim()))
        .unwrap_or_else(|| format!("deposit:{}:{}", auth.id, Uuid::new_v4()));
    let interest = maturity_interest(body.amount, body.term_days);
    // 结息模式：daily = 每日结息发到余额（到期只还本）；maturity = 到期一次性
    let mode: String = sqlx::query_scalar(
        "SELECT value FROM site_settings WHERE name = 'bank_fixed_settle_mode'",
    )
    .fetch_optional(&state.repo.db)
    .await
    .map_err(|e| DomainError::Internal(e.into()))?
    .unwrap_or_else(|| "maturity".into());
    let mode = if mode == "daily" { "daily" } else { "maturity" };
    // 单事务（P1 撕裂窗口收口）：扣款与存单落库同生共死。旧版扣款先提交、存单
    // INSERT 失败只能靠 spawn 退款补偿（随机幂等键），进程崩溃窗口内钱扣了无存单、
    // 无对账记录。同时幂等重放必须终止（P0）：重放时 spend 不再扣款，若继续落存单
    // = 免费多得一张存单、到期照付本息（对照 games scratch 的同款闸门）。
    let mut tx = state
        .repo
        .db
        .begin()
        .await
        .map_err(|e| DomainError::Internal(e.into()))?;
    if !matches!(
        spend_spark_tx(
            &mut tx,
            auth.id,
            body.amount,
            "bank_deposit",
            &idem,
            "bank",
            0,
        )
        .await?,
        SpendOutcome::Spent
    ) {
        return Err(DomainError::Validation(
            "该笔请求已受理，请勿重复提交".into(),
        ));
    }
    let id: i64 = sqlx::query_scalar(
        "INSERT INTO bank_deposits (user_id, amount, term_days, rate, interest, maturity_at, settle_mode) \
         VALUES ($1, $2, $3, $4, $5, now() + ($3 || ' days')::interval, $6) RETURNING id",
    )
    .bind(auth.id)
    .bind(body.amount)
    .bind(body.term_days)
    .bind(term_rate(body.term_days))
    .bind(interest)
    .bind(mode)
    .fetch_one(&mut *tx)
    .await
    .map_err(|e| DomainError::Internal(e.into()))?;
    tx.commit()
        .await
        .map_err(|e| DomainError::Internal(e.into()))?;

    Ok(ok(serde_json::json!({
        "id": id, "amount": body.amount, "term_days": body.term_days,
        "interest": interest, "rate": term_rate(body.term_days), "settle_mode": mode,
    })))
}

#[derive(sqlx::FromRow, serde::Serialize)]
struct DepositRow {
    id: i64,
    amount: i64,
    term_days: i32,
    interest: i64,
    paid_interest: i64,
    settle_mode: String,
    status: i16,
    maturity_at: chrono::DateTime<chrono::Utc>,
}

#[get("/bank/deposits")]
async fn bank_list(
    req: HttpRequest,
    state: web::Data<std::sync::Arc<AppState>>,
) -> DomainResult<impl Responder> {
    let auth = require_auth(&req, &state).await?;
    let rows = sqlx::query_as::<_, DepositRow>(
        "SELECT id, amount, term_days, interest, paid_interest, settle_mode, status, maturity_at FROM bank_deposits \
         WHERE user_id = $1 ORDER BY id DESC LIMIT 50",
    )
    .bind(auth.id)
    .fetch_all(&state.repo.db)
    .await
    .map_err(|e| DomainError::Internal(e.into()))?;
    Ok(ok(rows))
}

#[derive(Deserialize)]
struct WithdrawReq {
    deposit_id: i64,
}

#[post("/bank/withdraw")]
async fn bank_withdraw(
    req: HttpRequest,
    state: web::Data<std::sync::Arc<AppState>>,
    body: web::Json<WithdrawReq>,
) -> DomainResult<HttpResponse> {
    let auth = require_auth(&req, &state).await?;
    let d: Option<(
        i64,
        i64,
        i64,
        i64,
        String,
        i16,
        chrono::DateTime<chrono::Utc>,
    )> = sqlx::query_as(
        "SELECT id, amount, interest, paid_interest, settle_mode, status, maturity_at \
         FROM bank_deposits WHERE id = $1 AND user_id = $2",
    )
    .bind(body.deposit_id)
    .bind(auth.id)
    .fetch_optional(&state.repo.db)
    .await
    .map_err(|e| DomainError::Internal(e.into()))?;
    let Some((id, amount, interest, paid_interest, mode, status, maturity_at)) =
        d
    else {
        return Err(DomainError::NotFound(body.deposit_id));
    };
    if status != 0 {
        return Err(DomainError::Validation("该存款已处理".into()));
    }
    let bs = bank_settings(&state.repo.db).await;
    let matured = chrono::Utc::now() >= maturity_at;
    // maturity：到期本息全额；提前支取扣手续费、不计息。
    // daily：利息已按日发放（paid_interest），到期只还本；提前支取追回未到期部分利息防套利。
    let (payable, penalty, clawback) = if mode == "daily" {
        if matured {
            (amount, 0i64, 0i64)
        } else {
            let p = early_penalty(amount, bs.penalty_bp);
            (amount - p - paid_interest, p, paid_interest)
        }
    } else if matured {
        (amount + interest, 0, 0)
    } else {
        let p = early_penalty(amount, bs.penalty_bp);
        (amount - p, p, 0)
    };
    if payable < 0 {
        return Err(DomainError::Validation(
            "已发利息超过本金与手续费之和，无法支取，请联系管理员".into(),
        ));
    }

    // 单事务（P1 撕裂窗口收口）：销单 CAS 与本息入账同生共死。旧版 CAS 先提交、
    // earn 失败靠回滚状态位补偿——进程崩溃窗口内 status=1 锁死重试路径 = 本息蒸发。
    // 锁序对齐 worker bank_fixed_mature（先销单后入账），且共用 withdraw:{id} 幂等键，
    // 与到期自动兑付天然互斥双付。
    let idem = format!("withdraw:{}", id);
    let mut tx = state
        .repo
        .db
        .begin()
        .await
        .map_err(|e| DomainError::Internal(e.into()))?;
    let updated = sqlx::query(
        "UPDATE bank_deposits SET status = 1, settled_at = now(), penalty = $2, withdrawn_at = now() \
         WHERE id = $1 AND status = 0",
    )
    .bind(id)
    .bind(penalty)
    .execute(&mut *tx)
    .await
    .map_err(|e| DomainError::Internal(e.into()))?;
    if updated.rows_affected() == 0 {
        return Err(DomainError::LedgerConflict);
    }
    // 撕裂残留兜底：历史上「earn 已落账但状态位被回滚」的存单，此处 Replayed →
    // 仅补销单（本息已发过，不重复入账），属有意容忍的重放
    let earn_outcome =
        earn_spark_tx(&mut tx, auth.id, payable, "bank_withdraw", &idem)
            .await?;
    let _ = earn_outcome;
    tx.commit()
        .await
        .map_err(|e| DomainError::Internal(e.into()))?;
    Ok(ok(
        serde_json::json!({ "paid": payable, "matured": matured, "penalty": penalty,
            "clawback": clawback,
            "interest_earned": if mode != "daily" && matured { interest } else { 0 } }),
    ))
}

// ============ 活期与贷款（0048 火花银行对齐） ============

#[derive(Deserialize)]
struct DemandDepositReq {
    amount: i64,
    /// 客户端幂等键（审计修复 P1：防网络重试双扣；缺省回退随机键保持兼容）
    #[serde(default)]
    idempotency_key: Option<String>,
}

#[post("/bank/demand/deposit")]
async fn demand_deposit(
    req: HttpRequest,
    state: web::Data<std::sync::Arc<AppState>>,
    body: web::Json<DemandDepositReq>,
) -> DomainResult<HttpResponse> {
    let auth = require_auth(&req, &state).await?;
    if body.amount <= 0 {
        return Err(DomainError::Validation("存入金额必须为正".into()));
    }
    let bs = bank_settings(&state.repo.db).await;
    if body.amount < bs.min_demand {
        return Err(DomainError::Validation(format!(
            "活期单笔存入不少于 {} 魔力",
            bs.min_demand
        )));
    }
    let idem = body
        .idempotency_key
        .clone()
        .filter(|k| !k.trim().is_empty())
        .map(|k| format!("demand_in:{}:{}", auth.id, k.trim()))
        .unwrap_or_else(|| format!("demand_in:{}:{}", auth.id, Uuid::new_v4()));
    // 单事务（P1 撕裂窗口收口）：扣款与活期入账同生共死——旧版扣款先提交、入账失败
    // 只能靠 spawn 退款补偿（随机幂等键），进程崩溃窗口内钱扣了活期没涨。
    // 同时幂等重放必须终止（P0）：重放时 spend 不扣款，若继续累加活期余额 = 无中生有。
    let mut tx = state
        .repo
        .db
        .begin()
        .await
        .map_err(|e| DomainError::Internal(e.into()))?;
    if !matches!(
        spend_spark_tx(
            &mut tx,
            auth.id,
            body.amount,
            "bank_demand_in",
            &idem,
            "bank",
            0,
        )
        .await?,
        SpendOutcome::Spent
    ) {
        return Err(DomainError::Validation(
            "该笔请求已受理，请勿重复提交".into(),
        ));
    }
    sqlx::query(
        "INSERT INTO bank_demand_accounts (user_id, balance, daily_rate_bp, last_interest_date) \
         VALUES ($1, $2, $3, CURRENT_DATE) \
         ON CONFLICT (user_id) DO UPDATE SET balance = bank_demand_accounts.balance + $2, \
         daily_rate_bp = $3, updated_at = now()",
    )
    .bind(auth.id)
    .bind(body.amount)
    .bind(bs.demand_rate_bp)
    .execute(&mut *tx)
    .await
    .map_err(|e| DomainError::Internal(e.into()))?;
    tx.commit()
        .await
        .map_err(|e| DomainError::Internal(e.into()))?;
    Ok(ok(serde_json::json!({ "deposited": body.amount })))
}

#[derive(Deserialize)]
struct DemandWithdrawReq {
    amount: i64,
    /// 客户端幂等键（审计修复 P1：防网络重试双发入账；缺省回退随机键保持兼容）
    #[serde(default)]
    idempotency_key: Option<String>,
}

#[post("/bank/demand/withdraw")]
async fn demand_withdraw(
    req: HttpRequest,
    state: web::Data<std::sync::Arc<AppState>>,
    body: web::Json<DemandWithdrawReq>,
) -> DomainResult<HttpResponse> {
    let auth = require_auth(&req, &state).await?;
    if body.amount <= 0 {
        return Err(DomainError::Validation("支取金额必须为正".into()));
    }
    // 单事务（P1 撕裂窗口收口）：幂等检查、活期扣减、余额入账同生共死。旧版扣减先
    // 提交、earn 失败靠补偿 UPDATE 回补——进程崩溃窗口内钱从活期消失且无补偿；
    // 且幂等检查若后置于扣减，重试会再扣一次活期而入账侧被幂等键拦下 = 资金蒸发。
    let mut tx = state
        .repo
        .db
        .begin()
        .await
        .map_err(|e| DomainError::Internal(e.into()))?;
    let idem = body
        .idempotency_key
        .clone()
        .filter(|k| !k.trim().is_empty())
        .map(|k| format!("demand_out:{}:{}", auth.id, k.trim()))
        .unwrap_or_else(|| {
            format!("demand_out:{}:{}", auth.id, Uuid::new_v4())
        });
    // 幂等检查前置到扣减之前（同事务内）：重放请求直接拒绝
    let replayed: bool = sqlx::query_scalar(
        "SELECT EXISTS(SELECT 1 FROM spark_ledger WHERE idempotency_key = $1)",
    )
    .bind(&idem)
    .fetch_one(&mut *tx)
    .await
    .map_err(|e| DomainError::Internal(e.into()))?;
    if replayed {
        return Err(DomainError::Validation(
            "该笔支取已受理，请勿重复提交".into(),
        ));
    }
    let row: Option<i64> = sqlx::query_scalar(
        "UPDATE bank_demand_accounts SET balance = balance - $2, updated_at = now() \
         WHERE user_id = $1 AND balance >= $2 RETURNING balance",
    )
    .bind(auth.id)
    .bind(body.amount)
    .fetch_optional(&mut *tx)
    .await
    .map_err(|e| DomainError::Internal(e.into()))?;
    if row.is_none() {
        return Err(DomainError::Validation("活期余额不足".into()));
    }
    // 上方已在同事务内做过 EXISTS 重放预检，此处必为 Spent；显式丢弃以满足
    // must_use 契约（Replayed 只可能来自遗留撕裂残留，跳过入账即正确行为）
    let earn_outcome =
        earn_spark_tx(&mut tx, auth.id, body.amount, "bank_demand_out", &idem)
            .await?;
    let _ = earn_outcome;
    tx.commit()
        .await
        .map_err(|e| DomainError::Internal(e.into()))?;
    Ok(ok(
        serde_json::json!({ "paid": body.amount, "balance_left": row }),
    ))
}

#[derive(Deserialize)]
struct LoanApplyReq {
    amount: i64,
    term_days: i32,
}

/// 最大可贷额度 = 时魔/小时 × 系数 + 常数（时魔取自近 1 小时做种收益口径，无则 0）
async fn max_loan_amount(
    db: &PgPool,
    user_id: i64,
    bs: &BankSettings,
) -> DomainResult<i64> {
    let hourly: Option<i64> = sqlx::query_scalar(
        "SELECT COALESCE(sum(amount), 0)::bigint FROM spark_ledger \
         WHERE user_id = $1 AND kind = 'seeding_reward' AND created_at > now() - interval '1 hour'",
    )
    .bind(user_id)
    .fetch_one(db)
    .await
    .map_err(|e| DomainError::Internal(e.into()))?;
    Ok(hourly.unwrap_or(0) * bs.loan_ratio + bs.loan_constant)
}

#[post("/bank/loan/apply")]
async fn loan_apply(
    req: HttpRequest,
    state: web::Data<std::sync::Arc<AppState>>,
    body: web::Json<LoanApplyReq>,
) -> DomainResult<HttpResponse> {
    let auth = require_auth(&req, &state).await?;
    if !LOAN_TERMS.contains(&body.term_days) {
        return Err(DomainError::Validation(
            "贷款期限仅支持 7/30/90/180/365 天".into(),
        ));
    }
    let bs = bank_settings(&state.repo.db).await;
    if body.amount < bs.min_loan {
        return Err(DomainError::Validation(format!(
            "贷款金额不少于 {} 魔力",
            bs.min_loan
        )));
    }
    let balance: i64 =
        sqlx::query_scalar("SELECT spark_balance FROM users WHERE id = $1")
            .bind(auth.id)
            .fetch_one(&state.repo.db)
            .await
            .map_err(|e| DomainError::Internal(e.into()))?;
    if balance < 0 {
        return Err(DomainError::Validation(
            "当前魔力为负，暂不可申请贷款".into(),
        ));
    }
    let max = max_loan_amount(&state.repo.db, auth.id, &bs).await?;
    if body.amount > max {
        return Err(DomainError::Validation(format!(
            "贷款金额不可超过额度上限 {} 魔力（时魔 × {} + {}）",
            max, bs.loan_ratio, bs.loan_constant
        )));
    }
    let active: bool = sqlx::query_scalar(
        "SELECT EXISTS(SELECT 1 FROM bank_loans WHERE user_id = $1 AND status IN ('active', 'defaulted'))",
    )
    .bind(auth.id)
    .fetch_one(&state.repo.db)
    .await
    .map_err(|e| DomainError::Internal(e.into()))?;
    if active {
        return Err(DomainError::Validation(
            "已有未结清贷款，请先结清后再申请".into(),
        ));
    }
    let rate = loan_rate_bp(body.term_days);
    // 单事务（P1 撕裂窗口收口）：放款与建贷款行同生共死——旧版 earn 失败靠 DELETE
    // 补偿回滚贷款行，进程崩溃窗口内残留 active 贷款进入计息/逾期/自动扣款集合
    //（用户没收到钱却背上了债务）。
    let mut tx = state
        .repo
        .db
        .begin()
        .await
        .map_err(|e| DomainError::Internal(e.into()))?;
    let id: i64 = sqlx::query_scalar(
        "INSERT INTO bank_loans (user_id, amount, daily_rate_bp, penalty_rate_bp, term_days, remaining, due_at, last_interest_date) \
         VALUES ($1, $2, $3, $4, $5, $2, now() + ($5 || ' days')::interval, CURRENT_DATE) RETURNING id",
    )
    .bind(auth.id)
    .bind(body.amount)
    .bind(rate)
    .bind(bs.overdue_penalty_bp)
    .bind(body.term_days)
    .fetch_one(&mut *tx)
    .await
    .map_err(|e| DomainError::Internal(e.into()))?;
    let idem = format!("loan:{}:{}", auth.id, id);
    // 幂等键含新建贷款 id，重放不可达；显式丢弃以满足 must_use 契约
    let earn_outcome =
        earn_spark_tx(&mut tx, auth.id, body.amount, "bank_loan_payout", &idem)
            .await?;
    let _ = earn_outcome;
    tx.commit()
        .await
        .map_err(|e| DomainError::Internal(e.into()))?;
    Ok(ok(serde_json::json!({
        "id": id, "amount": body.amount, "term_days": body.term_days, "daily_rate_bp": rate,
        "due_in_days": body.term_days,
    })))
}

#[post("/bank/loan/repay")]
async fn loan_repay(
    req: HttpRequest,
    state: web::Data<std::sync::Arc<AppState>>,
) -> DomainResult<HttpResponse> {
    let auth = require_auth(&req, &state).await?;
    // 单事务（P1 收口）：旧版 FOR UPDATE 用在 autocommit 连接上——语句结束锁即释放，
    // payoff 按陈旧应计利息计算；扣款/销账两段提交靠退款补偿。现在锁、算、扣、销同事务。
    // 锁序先 users 后 loan（与 worker bank_auto_deduct 的 users→loan 一致，防互锁）。
    let mut tx = state
        .repo
        .db
        .begin()
        .await
        .map_err(|e| DomainError::Internal(e.into()))?;
    let _user_lock: i64 = sqlx::query_scalar(
        "SELECT spark_balance FROM users WHERE id = $1 FOR UPDATE",
    )
    .bind(auth.id)
    .fetch_one(&mut *tx)
    .await
    .map_err(|e| DomainError::Internal(e.into()))?;
    // 结清额 = 剩余本金 + 计提至今利息（含当日，一次性结清）
    let loan: Option<(i64, i64, i64, i64, chrono::NaiveDate)> = sqlx::query_as(
        "SELECT id, remaining, accrued_interest, daily_rate_bp, last_interest_date \
         FROM bank_loans WHERE user_id = $1 AND status IN ('active', 'defaulted') FOR UPDATE",
    )
    .bind(auth.id)
    .fetch_optional(&mut *tx)
    .await
    .map_err(|e| DomainError::Internal(e.into()))?;
    let Some((id, remaining, accrued, rate_bp, last_date)) = loan else {
        return Err(DomainError::Validation("没有进行中的贷款".into()));
    };
    let days = (chrono::Utc::now().date_naive() - last_date).num_days();
    let today_interest = if days > 0 {
        economy::loan_interest(remaining, rate_bp as i32, days)
    } else {
        0
    };
    let payoff = remaining + accrued + today_interest;
    let idem = format!("loan_repay:{}", id);
    // 幂等重放闸门（#[must_use] 连审）：重放不重复扣款
    if !matches!(
        spend_spark_tx(
            &mut tx,
            auth.id,
            payoff,
            "bank_loan_repay",
            &idem,
            "bank",
            id,
        )
        .await?,
        SpendOutcome::Spent
    ) {
        return Err(DomainError::Validation(
            "该笔还款已受理，请勿重复提交".into(),
        ));
    }
    // 销账校验影响行数（审计 P1）：与 worker bank_auto_deduct 并发时贷款可能已被结清。
    // 单事务下销账 0 行 = 整体回滚（扣款一并撤销），不再需要退款补偿路径。
    let settled = sqlx::query(
        "UPDATE bank_loans SET remaining = 0, accrued_interest = 0, status = 'paid', \
         paid_at = now(), last_interest_date = CURRENT_DATE WHERE id = $1 AND status IN ('active', 'defaulted')",
    )
    .bind(id)
    .execute(&mut *tx)
    .await
    .map_err(|e| DomainError::Internal(e.into()))?
    .rows_affected();
    if settled == 0 {
        return Err(DomainError::Validation(
            "贷款状态已变更（可能已被系统自动扣款结清），请刷新后重试".into(),
        ));
    }
    tx.commit()
        .await
        .map_err(|e| DomainError::Internal(e.into()))?;
    Ok(ok(serde_json::json!({
        "paid": payoff, "principal": remaining,
        "interest": accrued + today_interest,
    })))
}

#[derive(serde::Serialize, sqlx::FromRow)]
struct BankLoanRow {
    id: i64,
    amount: i64,
    daily_rate_bp: i32,
    term_days: i32,
    remaining: i64,
    accrued_interest: i64,
    status: String,
    due_at: chrono::DateTime<chrono::Utc>,
}

#[derive(serde::Serialize, sqlx::FromRow)]
struct DemandRow {
    balance: i64,
    daily_rate_bp: i32,
    last_interest_date: Option<chrono::NaiveDate>,
}

/// 银行总览：活期账户 + 资产汇总 + 当前贷款 + 额度
#[get("/bank/overview")]
async fn bank_overview(
    req: HttpRequest,
    state: web::Data<std::sync::Arc<AppState>>,
) -> DomainResult<impl Responder> {
    let auth = require_auth(&req, &state).await?;
    let bs = bank_settings(&state.repo.db).await;

    let demand: DemandRow = sqlx::query_as(
        "SELECT balance, daily_rate_bp, last_interest_date FROM bank_demand_accounts WHERE user_id = $1",
    )
    .bind(auth.id)
    .fetch_optional(&state.repo.db)
    .await
    .map_err(|e| DomainError::Internal(e.into()))?
    .unwrap_or(DemandRow { balance: 0, daily_rate_bp: bs.demand_rate_bp, last_interest_date: None });

    let fixed: Option<(i64, i64)> = sqlx::query_as::<_, (i64, i64)>(
        "SELECT COALESCE(sum(amount), 0)::bigint, count(*) FROM bank_deposits WHERE user_id = $1 AND status = 0",
    )
    .bind(auth.id)
    .fetch_optional(&state.repo.db)
    .await
    .map_err(|e| DomainError::Internal(e.into()))?;

    let loan: Option<BankLoanRow> = sqlx::query_as(
        // defaulted（逾期半期未还）也展示：用户需能看到被标记违约的贷款并还款
        "SELECT id, amount, daily_rate_bp, term_days, remaining, accrued_interest, status, due_at \
         FROM bank_loans WHERE user_id = $1 AND status IN ('active', 'defaulted')",
    )
    .bind(auth.id)
    .fetch_optional(&state.repo.db)
    .await
    .map_err(|e| DomainError::Internal(e.into()))?;

    let spark: i64 =
        sqlx::query_scalar("SELECT spark_balance FROM users WHERE id = $1")
            .bind(auth.id)
            .fetch_one(&state.repo.db)
            .await
            .map_err(|e| DomainError::Internal(e.into()))?;

    let loan_outstanding = loan
        .as_ref()
        .map(|l| l.remaining + l.accrued_interest)
        .unwrap_or(0);
    let total_asset = spark + demand.balance + fixed.unwrap_or((0, 0)).0;
    let max_loan = max_loan_amount(&state.repo.db, auth.id, &bs).await?;

    // 站点级运营概览 + 结息健康状态（对齐火花「站点银行概览/结息状态」）
    let site: (i64, i64, i64, i64, i64, i64, i64) = sqlx::query_as(
        "SELECT \
           (SELECT COALESCE(sum(balance), 0)::bigint FROM bank_demand_accounts), \
           (SELECT count(*) FROM bank_demand_accounts WHERE balance > 0), \
           (SELECT COALESCE(sum(amount), 0)::bigint FROM bank_deposits WHERE status = 0), \
           (SELECT count(*) FROM bank_deposits WHERE status = 0), \
           (SELECT COALESCE(sum(remaining + accrued_interest), 0)::bigint FROM bank_loans WHERE status = 'active'), \
           (SELECT count(*) FROM bank_loans WHERE status = 'active'), \
           (SELECT count(*) FROM bank_interest_records WHERE calc_date = CURRENT_DATE)",
    )
    .fetch_one(&state.repo.db)
    .await
    .map_err(|e| DomainError::Internal(e.into()))?;
    let last_run: Option<chrono::DateTime<chrono::Utc>> = sqlx::query_scalar(
        "SELECT finished_at FROM bank_settle_runs WHERE run_date = \
         ((CURRENT_TIMESTAMP + interval '8 hours')::date)",
    )
    .fetch_optional(&state.repo.db)
    .await
    .map_err(|e| DomainError::Internal(e.into()))?;
    let settle_mode: String = sqlx::query_scalar(
        "SELECT value FROM site_settings WHERE name = 'bank_fixed_settle_mode'",
    )
    .fetch_optional(&state.repo.db)
    .await
    .map_err(|e| DomainError::Internal(e.into()))?
    .unwrap_or_else(|| "maturity".into());

    Ok(ok(serde_json::json!({
        "spark_balance": spark,
        "demand": { "balance": demand.balance, "daily_rate_bp": demand.daily_rate_bp },
        "fixed": { "active_total": fixed.map(|f| f.0).unwrap_or(0), "active_count": fixed.map(|f| f.1).unwrap_or(0) },
        "loan": loan,
        "total_asset": total_asset,
        "net_asset": total_asset - loan_outstanding,
        "loan_outstanding": loan_outstanding,
        "max_loan": max_loan,
        "limits": {
            "min_deposit": bs.min_deposit, "max_deposit": bs.max_deposit,
            "min_demand": bs.min_demand, "min_loan": bs.min_loan,
            "penalty_bp": bs.penalty_bp,
        },
        "site": {
            "demand_total": site.0, "demand_count": site.1,
            "fixed_active_total": site.2, "fixed_count": site.3,
            "loan_outstanding_total": site.4, "loan_count": site.5,
            "today_interest_records": site.6,
            "settle_healthy": last_run.is_some(),
            "settle_mode": settle_mode,
        },
        "fixed_rates": VALID_TERMS.iter().map(|t| serde_json::json!({
            "term_days": t, "annual_rate": term_rate(*t),
        })).collect::<Vec<_>>(),
        "loan_rates": LOAN_TERMS.iter().map(|t| serde_json::json!({
            "term_days": t, "daily_rate_bp": loan_rate_bp(*t),
        })).collect::<Vec<_>>(),
    })))
}

// ============ 签到（M12） ============

#[post("/attendance/checkin")]
async fn checkin(
    req: HttpRequest,
    state: web::Data<std::sync::Arc<AppState>>,
) -> DomainResult<HttpResponse> {
    let auth = require_auth(&req, &state).await?;
    let today = (chrono::Utc::now() + chrono::Duration::hours(8)).date_naive(); // 站点时区 UTC+8
    let yesterday = today - chrono::Duration::days(1);

    // 幂等：今日已签直接返回
    let already: bool = sqlx::query_scalar(
        "SELECT EXISTS(SELECT 1 FROM attendance WHERE user_id = $1 AND date = $2)",
    )
    .bind(auth.id)
    .bind(today)
    .fetch_one(&state.repo.db)
    .await
    .unwrap_or(false);
    if already {
        return Err(DomainError::Validation("今天已经签到过啦".into()));
    }

    let last: Option<(chrono::NaiveDate, i32, i64)> = sqlx::query_as(
        // 跳过 makeup=true 的补签行读连签基线：admin 补签行 streak=0，
        // 若被当作「最后一条」会让下一次签到的 streak 错误重置为 1
        "SELECT date, streak, (count(*) OVER ())::bigint FROM attendance \
         WHERE user_id = $1 AND NOT makeup ORDER BY date DESC LIMIT 1",
    )
    .bind(auth.id)
    .fetch_optional(&state.repo.db)
    .await
    .map_err(|e| DomainError::Internal(e.into()))?;

    let (prev_date, prev_streak, total_days): (
        Option<chrono::NaiveDate>,
        i64,
        i64,
    ) = match last {
        Some((d, s, c)) => (Some(d), s as i64, c),
        None => (None, 0, 0),
    };
    let streak = if prev_date == Some(yesterday) {
        prev_streak + 1
    } else {
        1
    };
    let reward = checkin_reward(streak, total_days == 0);

    {
        // 单事务（P1 撕裂窗口收口）：签到行与本日奖励同生共死——旧版 attendance 落库
        // 成功后 earn 失败，当日奖励永久漏发（重试被「已签到」拦截，幂等键空有设计）。
        let mut tx = state
            .repo
            .db
            .begin()
            .await
            .map_err(|e| DomainError::Internal(e.into()))?;
        let inserted = sqlx::query(
            "INSERT INTO attendance (user_id, date, streak, reward) VALUES ($1, $2, $3, $4) \
             ON CONFLICT (user_id, date) DO NOTHING",
        )
        .bind(auth.id)
        .bind(today)
        .bind(streak)
        .bind(reward.total)
        .execute(&mut *tx)
        .await
        .map_err(|e| DomainError::Internal(e.into()))?;
        if inserted.rows_affected() == 0 {
            return Err(DomainError::Validation("今天已经签到过啦".into()));
        }
        let idem = format!("attendance:{}:{}", auth.id, today.format("%Y%m%d"));
        // 同事务内已由 ON CONFLICT DO NOTHING 保证首签唯一，此处必为 Spent；
        // 显式丢弃以满足 must_use 契约
        let earn_outcome =
            earn_spark_tx(&mut tx, auth.id, reward.total, "attendance", &idem)
                .await?;
        let _ = earn_outcome;
        tx.commit()
            .await
            .map_err(|e| DomainError::Internal(e.into()))?;
    }

    Ok(ok(serde_json::json!({
        "streak": reward.streak, "reward": reward.total,
        "base": reward.base, "streak_bonus": reward.streak_bonus,
    })))
}

#[get("/attendance")]
async fn checkin_status(
    req: HttpRequest,
    state: web::Data<std::sync::Arc<AppState>>,
) -> DomainResult<impl Responder> {
    let auth = require_auth(&req, &state).await?;
    let rows: Vec<(chrono::NaiveDate, i32, i64)> = sqlx::query_as(
        // 审计修复（P2）：补签行 makeup=true 且 streak=0，被 last() 当作最新连签会让首页
        // 在补签当天显示连签 0。recent 列表仍含补签行（日历要展示），streak 单独查非补签基线。
        "SELECT date, streak, reward FROM attendance WHERE user_id = $1 AND date >= current_date - interval '30 days' ORDER BY date",
    )
    .bind(auth.id)
    .fetch_all(&state.repo.db)
    .await
    .map_err(|e| DomainError::Internal(e.into()))?;
    let today = (chrono::Utc::now() + chrono::Duration::hours(8)).date_naive(); // 站点时区 UTC+8
    let checked_today = rows.iter().any(|(d, _, _)| *d == today);
    // streak 基线取最后一条非补签行（与签到主流程同口径），补签不重置显示
    let current_streak: i64 = sqlx::query_scalar(
        "SELECT streak FROM attendance WHERE user_id = $1 AND NOT makeup ORDER BY date DESC LIMIT 1",
    )
    .bind(auth.id)
    .fetch_optional(&state.repo.db)
    .await
    .map_err(|e| DomainError::Internal(e.into()))?
    .map(|s: i32| s as i64)
    .unwrap_or(0);
    // 补签卡持有数（未消耗订单；kind 兼容 makeup_card/resub_card，0066）
    let makeup_cards: i64 = sqlx::query_scalar(
        "SELECT count(*) FROM shop_orders o JOIN shop_items i ON i.id = o.item_id \
         WHERE o.user_id = $1 AND i.kind IN ('makeup_card','resub_card') \
           AND NOT EXISTS (SELECT 1 FROM resub_uses r WHERE r.idempotency_key = concat('resub:', o.id))",
    )
    .bind(auth.id)
    .fetch_one(&state.repo.db)
    .await
    .unwrap_or(0);
    Ok(ok(serde_json::json!({
        "checked_today": checked_today, "streak": current_streak,
        "recent": rows, "makeup_cards": makeup_cards,
    })))
}

// ============ 站免池（M13） ============

#[get("/magic-pool")]
async fn pool_status(
    state: web::Data<std::sync::Arc<AppState>>,
) -> DomainResult<impl Responder> {
    let month = economy::pool_month(chrono::Utc::now());
    let row: Option<(String, i64, i64, bool)> = sqlx::query_as(
        "SELECT month, donated_total, goal, promo_started FROM magic_pool WHERE month = $1",
    )
    .bind(&month)
    .fetch_optional(&state.repo.db)
    .await
    .map_err(|e| DomainError::Internal(e.into()))?;
    let (m, donated, goal, started) =
        row.unwrap_or((month.clone(), 0, economy::MAGIC_POOL_GOAL, false));
    let top: Vec<(String, i64)> = sqlx::query_as(
        "SELECT u.username, sum(d.amount)::bigint FROM pool_donations d \
         JOIN users u ON u.id = d.user_id WHERE d.month = $1 \
         GROUP BY u.username ORDER BY 2 DESC LIMIT 10",
    )
    .bind(&month)
    .fetch_all(&state.repo.db)
    .await
    .map_err(|e| DomainError::Internal(e.into()))?;
    Ok(ok(serde_json::json!({
        "month": m, "donated": donated, "goal": goal, "progress": if goal > 0 { donated as f64 / goal as f64 } else { 0.0 },
        "promo_started": started, "top_donors": top,
    })))
}

#[derive(Deserialize)]
struct DonateReq {
    amount: i64,
    /// 客户端幂等键（审计修复 P1：防网络重试双扣；缺省回退随机键保持兼容）
    #[serde(default)]
    idempotency_key: Option<String>,
}

#[post("/magic-pool/donate")]
async fn pool_donate(
    req: HttpRequest,
    state: web::Data<std::sync::Arc<AppState>>,
    body: web::Json<DonateReq>,
) -> DomainResult<HttpResponse> {
    let auth = require_auth(&req, &state).await?;
    if body.amount <= 0 {
        return Err(DomainError::Validation("捐赠金额必须为正".into()));
    }
    let month = economy::pool_month(chrono::Utc::now());
    let idem = body
        .idempotency_key
        .clone()
        .filter(|k| !k.trim().is_empty())
        .map(|k| format!("donate:{}:{}:{}", auth.id, month, k.trim()))
        .unwrap_or_else(|| {
            format!("donate:{}:{}:{}", auth.id, month, Uuid::new_v4())
        });
    // 审计修复（P1 非原子）：旧版 spend_spark 成功后 magic_pool / pool_donations 两段
    // INSERT 独立执行，失败即「钱扣了、池账与荣誉榜丢失」。改为单事务：扣款经
    // spend_spark_tx 与入池、流水三写同生共死，失败整体回滚（对照 funding_contribute）。
    let mut tx = state
        .repo
        .db
        .begin()
        .await
        .map_err(|e| DomainError::Internal(e.into()))?;
    // 幂等重放必须终止（P0）：重放时 spend 不扣款，继续累加池账/荣誉榜 = 捐赠翻倍，
    // 还可刷 donated_total 达标触发全站促销。对照 games scratch 的同款闸门。
    if !matches!(
        spend_spark_tx(
            &mut tx,
            auth.id,
            body.amount,
            "pool_donate",
            &idem,
            "pool",
            0,
        )
        .await?,
        SpendOutcome::Spent
    ) {
        return Err(DomainError::Validation(
            "该笔捐赠已受理，请勿重复提交".into(),
        ));
    }
    sqlx::query(
        "INSERT INTO magic_pool (month, donated_total) VALUES ($1, $2) \
         ON CONFLICT (month) DO UPDATE SET donated_total = magic_pool.donated_total + $2",
    )
    .bind(&month)
    .bind(body.amount)
    .execute(&mut *tx)
    .await
    .map_err(|e| DomainError::Internal(e.into()))?;
    sqlx::query("INSERT INTO pool_donations (user_id, amount, month) VALUES ($1, $2, $3)")
        .bind(auth.id)
        .bind(body.amount)
        .bind(&month)
        .execute(&mut *tx)
        .await
        .map_err(|e| DomainError::Internal(e.into()))?;
    tx.commit()
        .await
        .map_err(|e| DomainError::Internal(e.into()))?;
    state
        .repo
        .audit(Some(auth.id), "pool_donate", Some(body.amount))
        .await;
    Ok(ok(
        serde_json::json!({ "month": month, "donated": body.amount }),
    ))
}

// ============ M25 装扮中心 ============

#[derive(sqlx::FromRow, serde::Serialize)]
struct DressupRow {
    item_id: i64,
    name: String,
    kind: String,
    price: i64,
    slot: Option<String>,
    owned: bool,
    wearing: bool,
}

/// 装扮列表（拥有状态 + 佩戴中）
#[get("/dressup/list")]
async fn dressup_list(
    req: HttpRequest,
    state: web::Data<std::sync::Arc<AppState>>,
) -> DomainResult<impl Responder> {
    let auth = require_auth(&req, &state).await?;
    let rows: Vec<DressupRow> = sqlx::query_as(
        "SELECT si.id AS item_id, si.name, si.kind, si.price,             si.config->>'slot' AS slot,             EXISTS(SELECT 1 FROM user_dressups ud WHERE ud.user_id = $1 AND ud.item_id = si.id) AS owned,             COALESCE((SELECT ud.wearing FROM user_dressups ud WHERE ud.user_id = $1 AND ud.item_id = si.id), FALSE) AS wearing          FROM shop_items si          WHERE si.active AND si.kind IN ('avatar_frame','animated_avatar','rainbow_id','rainbow_name')          ORDER BY si.price",
    )
    .bind(auth.id)
    .fetch_all(&state.repo.db)
    .await
    .map_err(|e| DomainError::Internal(e.into()))?;
    Ok(ok(rows))
}

#[derive(Deserialize)]
struct WearReq {
    item_id: i64,
    wear: bool,
}

/// 佩戴/摘下（同类互斥由 DB 触发器保证：佩戴前先摘同类）
#[post("/dressup/wear")]
async fn dressup_wear(
    req: HttpRequest,
    state: web::Data<std::sync::Arc<AppState>>,
    body: web::Json<WearReq>,
) -> DomainResult<impl Responder> {
    let auth = require_auth(&req, &state).await?;
    // 槽位 + 装扮类型一起查：佩戴时按 kind 落到真实展示列（头像框→avatar_frame_id，
    // 动态头像→avatar_url 覆盖），否则装扮只在 user_dressups 里改标记、页面看不到效果
    let owned: Option<(Option<String>, String, Option<String>)> = sqlx::query_as(
        "SELECT si.config->>'slot', si.kind, si.config->>'avatar_url' FROM user_dressups ud JOIN shop_items si ON si.id = ud.item_id          WHERE ud.user_id = $1 AND ud.item_id = $2",
    )
    .bind(auth.id)
    .bind(body.item_id)
    .fetch_optional(&state.repo.db)
    .await
    .map_err(|e| DomainError::Internal(e.into()))?;
    let Some((slot, kind, effect_url)) = owned else {
        return Err(DomainError::Validation(
            "尚未拥有该装扮（先在商店购买）".into(),
        ));
    };

    if body.wear {
        // 先摘下同槽位（触发器互斥的最简前置）。注意 UPDATE...FROM 里目标表列不能带别名前缀
        sqlx::query(
            r#"UPDATE user_dressups ud SET wearing = FALSE FROM shop_items si
             WHERE si.id = ud.item_id AND ud.user_id = $1 AND si.config->>'slot' = $2"#,
        )
        .bind(auth.id)
        .bind(&slot)
        .execute(&state.repo.db)
        .await
        .map_err(|e| DomainError::Internal(e.into()))?;
        sqlx::query("UPDATE user_dressups SET wearing = TRUE WHERE user_id = $1 AND item_id = $2")
            .bind(auth.id)
            .bind(body.item_id)
            .execute(&state.repo.db)
            .await
            .map_err(|e| DomainError::Internal(e.into()))?;
        // 展示列落地（同槽位互斥）：avatar 槽二选一——
        //  avatar_frame     → users.avatar_frame_id（四季框/光环同链路，/me 与各页现查现回）
        //  animated_avatar  → users.avatar_url 覆盖为 config.avatar_url（gif 等动态图）
        match (kind.as_str(), slot.as_deref()) {
            ("avatar_frame", Some("avatar")) => {
                // config.avatar_frame_id 缺省时回落用价格最贵的一帧（管理员发放道具时可不填）
                let fid: Option<i32> = match effect_url.as_deref().and_then(|s| s.parse().ok()) {
                    Some(x) => Some(x),
                    None => sqlx::query_scalar(
                        "SELECT id FROM avatar_frames ORDER BY price DESC, id DESC LIMIT 1",
                    )
                    .fetch_optional(&state.repo.db)
                    .await
                    .map_err(|e| DomainError::Internal(e.into()))?
                    .flatten(),
                };
                sqlx::query(
                    "UPDATE users SET avatar_frame_id = $2 WHERE id = $1",
                )
                .bind(auth.id)
                .bind(fid)
                .execute(&state.repo.db)
                .await
                .map_err(|e| DomainError::Internal(e.into()))?;
            }
            ("animated_avatar", Some("avatar")) => {
                let Some(url) = effect_url.as_deref().filter(|s| !s.is_empty())
                else {
                    return Err(DomainError::Validation(
                        "该动态头像未配置图片（config.avatar_url），请联系管理员".into(),
                    ));
                };
                sqlx::query("UPDATE users SET avatar_url = $2 WHERE id = $1")
                    .bind(auth.id)
                    .bind(url)
                    .execute(&state.repo.db)
                    .await
                    .map_err(|e| DomainError::Internal(e.into()))?;
            }
            _ => {} // 用户名槽（rainbow_*）走样式列，暂无展示列需要落
        }
    } else {
        sqlx::query("UPDATE user_dressups SET wearing = FALSE WHERE user_id = $1 AND item_id = $2")
            .bind(auth.id)
            .bind(body.item_id)
            .execute(&state.repo.db)
            .await
            .map_err(|e| DomainError::Internal(e.into()))?;
        // 摘下恢复：动态头像恢复空值（回退首字母/原头像由用户自查），头像框清佩戴
        if kind == "animated_avatar" && slot.as_deref() == Some("avatar") {
            sqlx::query("UPDATE users SET avatar_url = NULL WHERE id = $1")
                .bind(auth.id)
                .execute(&state.repo.db)
                .await
                .map_err(|e| DomainError::Internal(e.into()))?;
        } else if kind == "avatar_frame" && slot.as_deref() == Some("avatar") {
            sqlx::query(
                "UPDATE users SET avatar_frame_id = NULL WHERE id = $1",
            )
            .bind(auth.id)
            .execute(&state.repo.db)
            .await
            .map_err(|e| DomainError::Internal(e.into()))?;
        }
    }
    state
        .repo
        .audit(
            Some(auth.id),
            if body.wear {
                "dressup.wear"
            } else {
                "dressup.takeoff"
            },
            Some(body.item_id),
        )
        .await;
    Ok(ok(
        serde_json::json!({ "item_id": body.item_id, "wearing": body.wear }),
    ))
}

// ============ 产出-回收对账（0077，v3 §27-22）+ Torznab 出口 ============

/// 火花产出/回收月度对账（staff）：通胀监控数据底座（v_spark_flow_monthly）
#[get("/admin/spark-flow")]
async fn spark_flow_report(
    req: HttpRequest,
    state: web::Data<std::sync::Arc<AppState>>,
) -> DomainResult<HttpResponse> {
    let auth = require_auth(&req, &state).await?;
    crate::authz::require_perm(
        &state,
        &auth,
        crate::authz::perm::SETTINGS_VIEW,
    )
    .await?;
    let rows: Vec<(String, i64, i64, i64, i64)> = sqlx::query_as(
        "SELECT month, minted::bigint, burned::bigint, net::bigint, entries FROM v_spark_flow_monthly LIMIT 24",
    )
    .fetch_all(&state.repo.db)
    .await
    .map_err(|e| DomainError::Internal(e.into()))?;
    Ok(ok(rows))
}

/// Torznab caps 端点（0077，cross-seed/Prowlarr 生态入口第一步）。
/// 0079：caps 补真实分类映射 + search 端点落地（映射到既有 torrents 搜索）。
#[get("/torznab")]
async fn torznab_caps() -> HttpResponse {
    let xml = r#"<?xml version="1.0" encoding="UTF-8"?>
<torznab:search xmlns:torznab="http://torznab.com/schemas/2015/feed">
  <server version="1.0" title="FluxTorrent" url="/api/v1/torznab" />
  <limits max="100" default="50" />
  <categories>
    <category id="8000" name="Other" />
    <category id="8001" name="Other/Education" />
  </categories>
  <search-fields>
    <field name="q" type="text" />
  </search-fields>
</torznab:search>"#;
    HttpResponse::Ok().content_type("application/xml").body(xml)
}

/// Torznab search（0079）：q=关键字 → 复用 TorrentFilter 的 trgm 搜索，atom 输出。
/// 鉴权与 compat 层同源：`Authorization: Token <api_token>`（开放 API Token）。
/// enclosure 指向 download.php（passkey 形状），Prowlarr/cross-seed 拿链接后带 passkey 拉取。
#[get("/torznab/search")]
async fn torznab_search(
    req: HttpRequest,
    state: web::Data<std::sync::Arc<AppState>>,
    q: web::Query<std::collections::HashMap<String, String>>,
) -> DomainResult<HttpResponse> {
    // 审计修复（P2）：item link/enclosure 需绝对地址，Prowlarr 等才能直接请求。
    // PUBLIC_API_URL 未配置时按请求 Host 拼。
    let api_base = std::env::var("PUBLIC_API_URL")
        .ok()
        .filter(|v| !v.trim().is_empty())
        .unwrap_or_else(|| {
            req.headers()
                .get("host")
                .and_then(|v| v.to_str().ok())
                .filter(|h| !h.is_empty())
                .map(|h| format!("http://{h}"))
                .unwrap_or_else(|| "http://127.0.0.1:8080".into())
        });
    let auth_uid = {
        // 审计修复（P1）：require_token 现返回 token 所属 user —— enclosure/link 的
        // passkey 占位符替换为该用户真实 passkey，否则 Prowlarr 拿到 PASSKEY 字面量必 401。
        let (uid, _) = crate::openapi_http::require_token(&req, &state).await?;
        let passkey: String =
            sqlx::query_scalar("SELECT passkey FROM users WHERE id = $1")
                .bind(uid)
                .fetch_one(&state.repo.db)
                .await
                .map_err(|e| DomainError::Internal(e.into()))?;
        passkey
    };
    let keyword = q.get("q").cloned().unwrap_or_default();
    let limit: usize = q
        .get("limit")
        .and_then(|v| v.parse().ok())
        .unwrap_or(50)
        .clamp(1, 100);
    let offset: i64 = q
        .get("offset")
        .and_then(|v| v.parse().ok())
        .unwrap_or(0)
        .max(0);
    let filter = crate::torrents::TorrentFilter {
        search: (!keyword.trim().is_empty())
            .then(|| keyword.trim().to_string()),
        ..Default::default()
    };
    // 翻页修复：list_torrents 内部把 limit clamp 到 50，offset≥50 时
    // 「取前 50 再 skip(50)」恒空。改为 offset ≤ 200 时按 offset+limit 原值直查
    // （绕过 clamp 的私有上限：传入的 limit 已在本端点 clamp(1,100)），
    // 更深翻页按 Torznab 惯例拒绝（Prowlarr 实际只翻到 1000）。
    let fetch_n = offset + limit as i64;
    if fetch_n > 1000 {
        return Err(DomainError::Validation(
            "offset+limit 不得超过 1000".into(),
        ));
    }
    let page = crate::torrents::list_torrents_noclamp(
        &state.repo.db,
        &filter,
        None,
        fetch_n,
    )
    .await?;
    let items: Vec<_> = page
        .items
        .into_iter()
        .skip(offset as usize)
        .take(limit)
        .map(|t| {
            let pub_date = t.created_at.to_rfc3339();
            // 促销标记（刷流时效性，NP 官方插件口径）：Torznab 标签 + 标题前缀双通道——
            // Prowlarr 按标签建索引，用户按标题肉眼/过滤规则识别。缺标记 = 免费信息断传。
            let promo_tag = match t.promotion.as_deref() {
                Some("free") => "[Free] ",
                Some("x2free") => "[2xFree] ",
                Some("half") => "[50%] ",
                Some("x2half") => "[2x50%] ",
                Some("x2") => "[2x] ",
                Some("p30") => "[30%] ",
                _ => "",
            };
            let torznab_tags = match t.promotion.as_deref() {
                Some(p @ ("free" | "x2free" | "half" | "x2half" | "x2" | "p30")) => {
                    format!("    <torznab:attr name=\"tags\" value=\"free,{p}\" />\n")
                }
                _ => String::new(),
            };
            format!(
                concat!(
                    "  <item>\n",
                    "    <title>{}{}</title>\n",
                    "    <guid isPermaLink=\"false\">torrent-{}</guid>\n",
                    "    <link>{}/api/v1/compat/nexusphp/download.php?id={}</link>\n",
                    "    <enclosure url=\"{}/api/v1/compat/nexusphp/download.php?id={}&amp;passkey={}\" type=\"application/x-bittorrent\" length=\"{}\" />\n",
                    "    <pubDate>{}</pubDate>\n",
                    "    <size>{}</size>\n",
                    "    <seeders>{}</seeders>\n",
                    "    <peers>{}</peers>\n",
                    "    <category id=\"8000\" name=\"Other\" />\n",
                    "{}",
                    "  </item>\n"
                ),
                promo_tag,
                xml_escape(&t.name),
                t.id,
                &api_base,
                t.id,
                &api_base,
                t.id,
                xml_escape(&auth_uid),
                t.size,
                pub_date,
                t.size,
                t.seeders,
                t.seeders + t.leechers,
                torznab_tags,
            )
        })
        .collect();
    let xml = format!(
        concat!(
            "<?xml version=\"1.0\" encoding=\"UTF-8\"?>\n",
            "<rss version=\"2.0\" xmlns:torznab=\"http://torznab.com/schemas/2015/feed\">\n",
            "<channel>\n",
            "  <title>FluxTorrent</title>\n",
            "  <description>FluxTorrent Torznab feed</description>\n",
            "{}",
            "</channel>\n",
            "</rss>\n"
        ),
        items.join("")
    );
    Ok(HttpResponse::Ok().content_type("application/xml").body(xml))
}

fn xml_escape(s: &str) -> String {
    s.replace('&', "&amp;")
        .replace('<', "&lt;")
        .replace('>', "&gt;")
        .replace('"', "&quot;")
}

// ============ 定向众筹免费（0078，HDBits Featured 口径，v3 §27-13） ============
// 某个种子社区凑火花 → 达标自动挂 free×hours；到期未达标全额退款（含税部分一并退）。
// 赠送税（gift_tax_bp）对众筹同样适用：抽税入站免池，与礼物共用回收通道（v3 §27-22）。

#[derive(sqlx::FromRow, serde::Serialize)]
struct FundingRow {
    id: i64,
    torrent_id: i64,
    torrent_name: Option<String>,
    goal: i64,
    raised: i64,
    backers: i64,
    hours: i32,
    status: i16,
    ends_at: chrono::DateTime<chrono::Utc>,
}

/// 进行中/已完成的众筹列表（新→旧；种子名带出）
#[get("/fundings")]
async fn fundings_list(
    state: web::Data<std::sync::Arc<AppState>>,
    q: web::Query<std::collections::HashMap<String, String>>,
) -> DomainResult<impl Responder> {
    let status: Option<i16> = q
        .get("status")
        .and_then(|s| s.parse::<i16>().ok())
        .filter(|s| (0..=3).contains(s));
    let rows = sqlx::query_as::<_, FundingRow>(
        "SELECT f.id, f.torrent_id, t.name AS torrent_name, f.goal, f.raised, \
                (SELECT count(*)::bigint FROM funding_contribs c WHERE c.funding_id = f.id) AS backers, \
                f.hours, f.status, f.ends_at \
         FROM fundings f LEFT JOIN torrents t ON t.id = f.torrent_id \
         WHERE ($1::smallint IS NULL OR f.status = $1) \
         ORDER BY f.status ASC, f.ends_at DESC LIMIT 50",
    )
    .bind(status)
    .fetch_all(&state.repo.db)
    .await
    .map_err(|e| DomainError::Internal(e.into()))?;
    Ok(ok(rows))
}

#[derive(Deserialize)]
struct FundingCreateReq {
    torrent_id: i64,
    goal: i64,
    #[serde(default = "default_funding_hours")]
    hours: i32,
    #[serde(default = "default_funding_days")]
    days: i32,
}

fn default_funding_hours() -> i32 {
    168
}
fn default_funding_days() -> i32 {
    14
}

/// 发起众筹（种子发布者或 staff；同种子同时只能有一个进行中的众筹）
#[post("/fundings")]
async fn funding_create(
    req: HttpRequest,
    state: web::Data<std::sync::Arc<AppState>>,
    body: web::Json<FundingCreateReq>,
) -> DomainResult<HttpResponse> {
    let auth = require_auth(&req, &state).await?;
    if body.goal < 1000 {
        return Err(DomainError::Validation("众筹目标至少 1000 魔力".into()));
    }
    if !(1..=720).contains(&body.hours) || !(1..=60).contains(&body.days) {
        return Err(DomainError::Validation(
            "hours 需在 1-720、days 需在 1-60 之间".into(),
        ));
    }
    let torrent: Option<(i64, i16)> = sqlx::query_as(
        "SELECT owner_id, approval_status FROM torrents WHERE id = $1",
    )
    .bind(body.torrent_id)
    .fetch_optional(&state.repo.db)
    .await
    .map_err(|e| DomainError::Internal(e.into()))?;
    let Some((owner_id, _)) = torrent else {
        return Err(DomainError::NotFound(body.torrent_id));
    };
    if auth.id != owner_id && auth.class_id < 90 {
        return Err(DomainError::Forbidden);
    }
    let open: bool = sqlx::query_scalar(
        "SELECT EXISTS(SELECT 1 FROM fundings WHERE torrent_id = $1 AND status = 0)",
    )
    .bind(body.torrent_id)
    .fetch_one(&state.repo.db)
    .await
    .map_err(|e| DomainError::Internal(e.into()))?;
    if open {
        return Err(DomainError::Validation("该种子已有进行中的众筹".into()));
    }
    let id: i64 = sqlx::query_scalar(
        "INSERT INTO fundings (torrent_id, creator_id, goal, hours, ends_at) \
         VALUES ($1, $2, $3, $4, now() + make_interval(days => $5)) RETURNING id",
    )
    .bind(body.torrent_id)
    .bind(auth.id)
    .bind(body.goal)
    .bind(body.hours)
    .bind(body.days)
    .fetch_one(&state.repo.db)
    .await
    .map_err(|e| DomainError::Internal(e.into()))?;
    state
        .repo
        .audit(Some(auth.id), "funding_create", Some(id))
        .await;
    Ok(ok(
        serde_json::json!({ "id": id, "goal": body.goal, "hours": body.hours }),
    ))
}

#[derive(Deserialize)]
struct FundingContributeReq {
    funding_id: i64,
    amount: i64,
    #[serde(default)]
    idempotency_key: Option<String>,
}

/// 参与众筹（一人一项目可追投；扣款走 spend_spark 幂等，税入池、净额计入 raised）。
/// 退款时按实付全额退（税部分由站免池承担——池子本来就是回收通道）。
#[post("/fundings/contribute")]
async fn funding_contribute(
    req: HttpRequest,
    state: web::Data<std::sync::Arc<AppState>>,
    body: web::Json<FundingContributeReq>,
) -> DomainResult<HttpResponse> {
    let auth = require_auth(&req, &state).await?;
    if body.amount <= 0 {
        return Err(DomainError::Validation("参与金额必须为正".into()));
    }
    let f: Option<(i64, i16)> = sqlx::query_as(
        "SELECT goal, status FROM fundings WHERE id = $1 AND ends_at > now()",
    )
    .bind(body.funding_id)
    .fetch_optional(&state.repo.db)
    .await
    .map_err(|e| DomainError::Internal(e.into()))?;
    let Some((goal, status)) = f else {
        return Err(DomainError::NotFound(body.funding_id));
    };
    if status != 0 {
        return Err(DomainError::Validation("众筹已结束".into()));
    }
    // 幂等键必须带 uid 前缀（P0）：裸客户端键会跨用户碰撞——A 占用键 X 后，B 用 X
    // 参与会被判为重放而不扣款，contribs 照记、到期按全额退款 = 提现通道。
    // 与 bank/donate/games 全部同口径。
    let idem = body
        .idempotency_key
        .clone()
        .filter(|k| !k.trim().is_empty())
        .map(|k| format!("funding:{}:{}", auth.id, k.trim()))
        .ok_or(DomainError::Validation("缺少 idempotency_key".into()))?;
    // 税：基点可调（site_settings gift_tax_bp，缺省 500=5%）；0=免税
    let tax_bp: i32 = sqlx::query_scalar(
        "SELECT value FROM site_settings WHERE name = 'gift_tax_bp'",
    )
    .fetch_optional(&state.repo.db)
    .await
    .map_err(|e| DomainError::Internal(e.into()))?
    .and_then(|v: String| v.parse().ok())
    .unwrap_or(500);
    let tax = economy::gift_tax(body.amount, tax_bp);
    let net = body.amount - tax;
    // 幂等重放必须终止（P0）：重放时 spend 不扣款，若继续累加 raised/contribs =
    // 众筹虚增达标白嫖免费促销，到期还能按 contribs 全额退款。
    if !matches!(
        crate::economy_http::spend_spark(
            &state.repo.db,
            auth.id,
            body.amount,
            "funding",
            &idem,
            "funding",
            body.funding_id,
        )
        .await?,
        SpendOutcome::Spent
    ) {
        return Err(DomainError::Validation(
            "该笔参与已受理，请勿重复提交".into(),
        ));
    }
    // 参与记录 + 进度推进（审计 P1-5：旧版三段独立语句，扣款成功但 contribs 落库
    // 失败时该笔不在退款集合——worker 按 funding_contribs 逐行退，钱有去无回。
    // 现在两段进同一事务，任一失败整体回滚并冲销扣款。）
    let contrib_ok = async {
        let mut tx = state.repo.db.begin().await.map_err(|e| e.to_string())?;
        sqlx::query(
            "INSERT INTO funding_contribs (funding_id, user_id, amount, tax) VALUES ($1, $2, $3, $4)              ON CONFLICT (funding_id, user_id) DO UPDATE              SET amount = funding_contribs.amount + EXCLUDED.amount,                  tax = funding_contribs.tax + EXCLUDED.tax, created_at = now()",
        )
        .bind(body.funding_id)
        .bind(auth.id)
        .bind(body.amount)
        .bind(tax)
        .execute(&mut *tx)
        .await
        .map_err(|e| e.to_string())?;
        sqlx::query("UPDATE fundings SET raised = raised + $2 WHERE id = $1")
            .bind(body.funding_id)
            .bind(net)
            .execute(&mut *tx)
            .await
            .map_err(|e| e.to_string())?;
        tx.commit().await.map_err(|e| e.to_string())
    }
    .await;
    if let Err(why) = contrib_ok {
        let db = state.repo.db.clone();
        let uid = auth.id;
        let amount = body.amount;
        let idem2 = format!("funding_refund:{}", Uuid::new_v4());
        actix_web::rt::spawn(async move {
            let _ = crate::economy_http::earn_spark(
                &db,
                uid,
                amount,
                "funding_refund",
                &idem2,
            )
            .await;
        });
        return Err(DomainError::Validation(format!(
            "参与记录写入失败，已发起退款冲销：{why}"
        )));
    }
    // 税入站免池（与 pool_donate 同账：magic_pool + pool_donations）。
    // 账务口径：税不另记 spark_ledger——支出方的 -amount 流水已把含税全额记为回收，
    // 这里只入池账；若再向某个汇入账户记正流水会虚增 v_spark_flow_monthly 的 minted。
    if tax > 0 {
        let month = economy::pool_month(chrono::Utc::now());
        sqlx::query(
            "INSERT INTO magic_pool (month, donated_total) VALUES ($1, $2) \
             ON CONFLICT (month) DO UPDATE SET donated_total = magic_pool.donated_total + $2",
        )
        .bind(&month)
        .bind(tax)
        .execute(&state.repo.db)
        .await
        .map_err(|e| DomainError::Internal(e.into()))?;
        sqlx::query("INSERT INTO pool_donations (user_id, amount, month) VALUES ($1, $2, $3)")
            .bind(auth.id)
            .bind(tax)
            .bind(&month)
            .execute(&state.repo.db)
            .await
            .map_err(|e| DomainError::Internal(e.into()))?;
    }
    state
        .repo
        .audit(Some(auth.id), "funding_contribute", Some(body.funding_id))
        .await;
    let raised: i64 =
        sqlx::query_scalar("SELECT raised FROM fundings WHERE id = $1")
            .bind(body.funding_id)
            .fetch_one(&state.repo.db)
            .await
            .map_err(|e| DomainError::Internal(e.into()))?;
    Ok(ok(serde_json::json!({
        "funding_id": body.funding_id, "paid": body.amount, "tax": tax,
        "raised": raised, "goal": goal, "reached": raised >= goal,
    })))
}

/// 我的参与记录
#[get("/fundings/mine")]
async fn funding_my(
    req: HttpRequest,
    state: web::Data<std::sync::Arc<AppState>>,
) -> DomainResult<impl Responder> {
    let auth = require_auth(&req, &state).await?;
    let rows: Vec<(i64, i64, i64, i16, i64)> = sqlx::query_as(
        "SELECT f.id, c.amount, c.tax, f.status, c.funding_id \
         FROM funding_contribs c JOIN fundings f ON f.id = c.funding_id \
         WHERE c.user_id = $1 ORDER BY c.created_at DESC LIMIT 100",
    )
    .bind(auth.id)
    .fetch_all(&state.repo.db)
    .await
    .map_err(|e| DomainError::Internal(e.into()))?;
    Ok(ok(rows))
}
