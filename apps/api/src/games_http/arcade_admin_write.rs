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
pub(super) async fn anchor_of(db: &sqlx::PgPool, key: Option<&str>) -> i64 {
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

pub(super) fn default_kind() -> String {
    "magic".to_string()
}
pub(super) fn default_qty() -> i32 {
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
