//! 娱乐屋后台**写**侧：奖池保存与物品目录保存。
//!
//! 从只读面板 arcade_admin 拆出：读面板是「展示现值」，写面板是「改现值并自证合法」，
//! 两者的失败模式与测试关注点不同，混在一个文件里会同时撞行数与行宽上限。
//!
//! 共同纪律：**关闸放在写侧** —— 不合法就拒绝保存并整体回滚，
//! 坏配置根本进不了表。只靠读侧关闸等于让面板自己成为一条后门。

use actix_web::{post, web, HttpRequest, HttpResponse};
use serde::Deserialize;
use serde_json::json;

use crate::authz;
use crate::dto::ok;
use crate::errors::{DomainError, DomainResult};
use crate::games;
use crate::http::require_auth;
use crate::state::AppState;

/// sqlx 错误只实现了 From<anyhow::Error>，这里显式转一层，不让它冒到 `?` 上
fn dberr(e: sqlx::Error) -> DomainError {
    DomainError::Internal(anyhow::Error::from(e))
}

/// 取物品的权威折算价。停用/不存在 → 0，交给 validate_pool 当空头承诺拒掉；
/// 这里**不**回落登记价也不猜数 —— 「价值」是 EV 的分母，猜一个就是把闸让出去。
async fn anchor_of(db: &sqlx::PgPool, key: Option<&str>) -> i64 {
    match key {
        Some(k) => sqlx::query_scalar(
            "SELECT anchor FROM arcade_items WHERE key = $1 AND enabled",
        )
        .bind(k)
        .fetch_optional(db)
        .await
        .unwrap_or(None)
        .unwrap_or(0),
        None => 0,
    }
}

fn default_kind() -> String {
    "magic".to_string()
}
fn default_qty() -> i32 {
    1
}

#[derive(Deserialize)]
pub(super) struct PoolEntryReq {
    pub label: String,
    pub weight: i32,
    #[serde(default)]
    pub payout: i64,
    /// magic | item；缺省 magic，老面板不传也能存
    #[serde(default = "default_kind")]
    pub kind: String,
    #[serde(default)]
    pub item_key: Option<String>,
    #[serde(default = "default_qty")]
    pub qty: i32,
    #[serde(default)]
    pub enabled: bool,
}

#[derive(Deserialize)]
pub(super) struct PoolSaveReq {
    pub pool_key: String,
    pub game: String,
    pub label: String,
    pub ticket: i64,
    pub entries: Vec<PoolEntryReq>,
}

#[derive(Deserialize)]
pub(super) struct ItemSaveReq {
    pub key: String,
    pub name: String,
    pub kind: String,
    pub anchor: i64,
    pub anchor_src: String,
    #[serde(default = "default_true")]
    pub unlimited: bool,
    #[serde(default)]
    pub stock: i64,
    #[serde(default = "default_qty")]
    pub per_user: i32,
    #[serde(default)]
    pub icon: String,
    #[serde(default = "default_true")]
    pub enabled: bool,
}

fn default_true() -> bool {
    true
}

/// 保存物品目录项。**改一个物品的 anchor 会同时改掉所有引用它的奖池的 EV**，
/// 所以写侧必须回查这些池：否则面板本身就是一条绕过关闸的后门。
#[post("/admin/arcade/items")]
pub(super) async fn arcade_item_save(
    req: HttpRequest,
    state: web::Data<std::sync::Arc<AppState>>,
    body: web::Json<ItemSaveReq>,
) -> DomainResult<HttpResponse> {
    let auth = require_auth(&req, &state).await?;
    authz::require_perm(&state, &auth, authz::perm::USER_ADJUST).await?;
    let b = body.into_inner();
    let db = &state.repo.db;

    if b.key.trim().is_empty() || b.name.trim().is_empty() {
        return Err(DomainError::Validation("key / name 不能为空".into()));
    }
    if !matches!(b.kind.as_str(), "economic" | "voucher" | "cosmetic") {
        return Err(DomainError::Validation(format!(
            "kind 只能是 economic | voucher | cosmetic，收到「{}」",
            b.kind
        )));
    }
    if !matches!(
        b.anchor_src.as_str(),
        "shop" | "derived" | "declared" | "n/a"
    ) {
        return Err(DomainError::Validation(format!(
            "anchor_src 只能是 shop | derived | declared | n/a，收到「{}」",
            b.anchor_src
        )));
    }
    if b.per_user <= 0 {
        return Err(DomainError::Validation("每人上限必须为正".into()));
    }
    // 经济类物品必须报得出正折算价，否则「零负债」是假的
    if b.kind == "economic" && b.anchor <= 0 {
        return Err(DomainError::Validation(format!(
            "economic 物品「{}」anchor 必须为正：它按定义就是站点负债",
            b.key
        )));
    }
    if !b.unlimited && b.stock < 0 {
        return Err(DomainError::Validation("限量物品 stock 不能为负".into()));
    }

    let mut tx = db.begin().await.map_err(dberr)?;
    sqlx::query(
        r#"
        INSERT INTO arcade_items
            (key, name, kind, anchor, anchor_src,
             unlimited, stock, per_user, icon, enabled)
        VALUES ($1, $2, $3, $4, $5, $6, $7, $8, $9, $10)
        ON CONFLICT (key) DO UPDATE SET
            name = EXCLUDED.name, kind = EXCLUDED.kind,
            anchor = EXCLUDED.anchor, anchor_src = EXCLUDED.anchor_src,
            unlimited = EXCLUDED.unlimited, stock = EXCLUDED.stock,
            per_user = EXCLUDED.per_user, icon = EXCLUDED.icon,
            enabled = EXCLUDED.enabled, updated_at = now()
    "#,
    )
    .bind(&b.key)
    .bind(&b.name)
    .bind(&b.kind)
    .bind(b.anchor)
    .bind(&b.anchor_src)
    .bind(b.unlimited)
    .bind(b.stock)
    .bind(b.per_user)
    .bind(&b.icon)
    .bind(b.enabled)
    .execute(&mut *tx)
    .await
    .map_err(dberr)?;

    // 回查：所有引用这件物品的池子，按**新 anchor** 重新校验 EV。
    // 不合法就整个回滚 —— 不允许「物品存下来了、池子却悄悄变成增发」。
    let pools: Vec<(String, i64)> = sqlx::query_as(
        r#"
        SELECT DISTINCT p.key, p.ticket
        FROM arcade_pools p
        JOIN arcade_pool_entries e ON e.pool_key = p.key
        WHERE e.item_key = $1 AND p.enabled AND e.enabled
    "#,
    )
    .bind(&b.key)
    .fetch_all(&mut *tx)
    .await
    .map_err(dberr)?;
    for (pk, ticket) in pools {
        let rows: Vec<(
            String,
            i32,
            i64,
            String,
            Option<String>,
            i32,
            Option<i64>,
        )> = sqlx::query_as(
            r#"
            SELECT e.label, e.weight, e.payout, e.kind,
                   e.item_key, e.qty, i.anchor
            FROM arcade_pool_entries e
            LEFT JOIN arcade_items i
                   ON i.key = e.item_key AND i.enabled
            WHERE e.pool_key = $1 AND e.enabled
            ORDER BY e.sort
        "#,
        )
        .bind(&pk)
        .fetch_all(&mut *tx)
        .await
        .map_err(dberr)?;
        let entries: Vec<games::PoolEntry> = rows
            .into_iter()
            .map(|(label, weight, payout, kind, ik, qty, anchor)| {
                games::PoolEntry {
                    label,
                    weight: u32::try_from(weight.max(0)).unwrap_or(u32::MAX),
                    kind: if kind == "item" {
                        games::EntryKind::Item {
                            item_key: ik.unwrap_or_default(),
                            qty: qty.max(1),
                            anchor: anchor.unwrap_or(0),
                        }
                    } else {
                        games::EntryKind::Magic { multiples: payout }
                    },
                }
            })
            .collect();
        games::validate_pool(&entries, ticket).map_err(|e| {
            DomainError::Validation(format!(
                "物品改动会让奖池「{pk}」不合法，已回滚：{e}"
            ))
        })?;
    }
    tx.commit().await.map_err(dberr)?;
    state
        .repo
        .audit(Some(auth.id), "arcade.item.save", None)
        .await;
    Ok(ok(
        json!({ "key": b.key, "anchor": b.anchor, "kind": b.kind }),
    ))
}

/// 保存奖池。**关闸放在写侧**：不合法就拒绝保存，坏池子根本进不了表。
/// 只靠读侧关闸等于让面板有能力一键把玩法打成 503 —— 那是把正确性换成事故。
#[post("/admin/arcade/pool")]
pub(super) async fn arcade_pool_save(
    req: HttpRequest,
    state: web::Data<std::sync::Arc<AppState>>,
    body: web::Json<PoolSaveReq>,
) -> DomainResult<HttpResponse> {
    let auth = require_auth(&req, &state).await?;
    authz::require_perm(&state, &auth, authz::perm::USER_ADJUST).await?;
    let b = body.into_inner();
    if b.pool_key.trim().is_empty() || b.game.trim().is_empty() {
        return Err(DomainError::Validation("pool_key / game 不能为空".into()));
    }
    if b.ticket <= 0 {
        return Err(DomainError::Validation("票价必须为正".into()));
    }
    // 先逐个解析物品 anchor 再校验：anchor 要查库，闭包里 await 不合法，
    // 而且「先全解析、再一次校验」才能保证坏配置一条都不会写进去。
    let mut entries: Vec<games::PoolEntry> = Vec::new();
    for e in b.entries.iter().filter(|e| e.enabled) {
        let kind = if e.kind == "item" {
            games::EntryKind::Item {
                item_key: e.item_key.clone().unwrap_or_default(),
                qty: e.qty.max(1),
                anchor: anchor_of(&state.repo.db, e.item_key.as_deref()).await,
            }
        } else {
            games::EntryKind::Magic {
                multiples: e.payout,
            }
        };
        entries.push(games::PoolEntry {
            label: e.label.clone(),
            weight: u32::try_from(e.weight.max(0)).unwrap_or(u32::MAX),
            kind,
        });
    }
    games::validate_pool(&entries, b.ticket).map_err(|e| {
        DomainError::Validation(format!("奖池不合法，已拒绝保存：{e}"))
    })?;

    let mut tx = state.repo.db.begin().await.map_err(dberr)?;
    sqlx::query(
        r#"
        INSERT INTO arcade_pools (key, game, label, ticket)
        VALUES ($1, $2, $3, $4)
        ON CONFLICT (key) DO UPDATE SET
            game = EXCLUDED.game, label = EXCLUDED.label,
            ticket = EXCLUDED.ticket, updated_at = now()
    "#,
    )
    .bind(&b.pool_key)
    .bind(&b.game)
    .bind(&b.label)
    .bind(b.ticket)
    .execute(&mut *tx)
    .await
    .map_err(dberr)?;
    sqlx::query("DELETE FROM arcade_pool_entries WHERE pool_key = $1")
        .bind(&b.pool_key)
        .execute(&mut *tx)
        .await
        .map_err(dberr)?;
    for (i, e) in b.entries.iter().enumerate() {
        sqlx::query(
            r#"
            INSERT INTO arcade_pool_entries
                (pool_key, label, weight, payout, kind,
                 item_key, qty, enabled, sort)
            VALUES ($1, $2, $3, $4, $5, $6, $7, $8, $9)
        "#,
        )
        .bind(&b.pool_key)
        .bind(&e.label)
        .bind(e.weight)
        .bind(if e.kind == "item" { 0 } else { e.payout })
        .bind(if e.kind == "item" { "item" } else { "magic" })
        .bind(if e.kind == "item" {
            e.item_key.as_deref()
        } else {
            None
        })
        .bind(if e.kind == "item" { e.qty.max(1) } else { 1 })
        .bind(e.enabled)
        .bind(i as i32)
        .execute(&mut *tx)
        .await
        .map_err(dberr)?;
    }
    tx.commit().await.map_err(dberr)?;
    state
        .repo
        .audit(Some(auth.id), "arcade.pool.save", None)
        .await;
    Ok(ok(json!({
        "pool": b.pool_key,
        "entries": b.entries.len(),
        "ev": games::pool_ev(&entries, b.ticket),
    })))
}
