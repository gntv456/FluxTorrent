//! 物品目录保存（后台写侧）。
//!
//! 从 arcade_admin_write 拆出：两个写端点的**校验面不同** ——
//! 改一个物品的 anchor 会连带改掉所有引用它的池的 EV，
//! 必须跨池回查并整体回滚。这条逻辑独立出来才看得清，
//! 也不会把两个端点的行数叠在一起撞 300 上限。

use actix_web::{delete, post, web, HttpRequest, HttpResponse};
use serde::Deserialize;
use serde_json::json;

use crate::authz;
use crate::dto::ok;
use crate::errors::{DomainError, DomainResult};
use crate::games;
use crate::http::require_auth;
use crate::state::AppState;

use super::arcade_admin_write::default_qty;
use super::pool::dberr;
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

/// 删除目录项。**两道必须先查，否则删除本身就是炸玩法读表的路**：
///  · 还有奖池位引用它 —— `arcade_pool_entries.item_key` 是
///    `ON DELETE SET NULL`，而 CHECK 要求 `kind='item'` 时 item_key 非空，
///    删掉之后 load_pool 会当场校验失败，玩法直接拒绝服务；
///  · 发放账里有它 —— 余量与每人上限都由 `arcade_item_grants` 反推，
///    删物品会 CASCADE 清账，等于把「发出去多少」的证据抹掉。有账只许停用。
#[delete("/admin/arcade/items/{key}")]
pub(super) async fn arcade_item_delete(
    req: HttpRequest,
    state: web::Data<std::sync::Arc<AppState>>,
    path: web::Path<String>,
) -> DomainResult<HttpResponse> {
    let auth = require_auth(&req, &state).await?;
    authz::require_perm(&state, &auth, authz::perm::USER_ADJUST).await?;
    let key = path.into_inner();
    let db = &state.repo.db;

    let name: Option<String> =
        sqlx::query_scalar("SELECT name FROM arcade_items WHERE key = $1")
            .bind(&key)
            .fetch_optional(db)
            .await
            .map_err(dberr)?;
    let name = name.ok_or_else(|| {
        DomainError::Validation(format!("目录里没有「{key}」"))
    })?;

    let refs: Vec<(String, String)> = sqlx::query_as(
        r#"
        SELECT e.label, p.key
          FROM arcade_pool_entries e
          JOIN arcade_pools p ON p.key = e.pool_key
         WHERE e.item_key = $1 AND p.enabled AND e.enabled
         ORDER BY p.key, e.sort
        "#,
    )
    .bind(&key)
    .fetch_all(db)
    .await
    .map_err(dberr)?;
    if !refs.is_empty() {
        let spots: Vec<String> = refs
            .iter()
            .map(|(label, pk)| format!("{pk}·{label}"))
            .collect();
        return Err(DomainError::Validation(format!(
            "「{name}」仍被 {} 个奖池位引用（{}）：先从奖池撤下再删",
            refs.len(),
            spots.join("、")
        )));
    }

    let grants: (i64, i64) = sqlx::query_as(
        r#"
        SELECT count(*)::bigint, COALESCE(sum(qty), 0)::bigint
          FROM arcade_item_grants WHERE item_key = $1
        "#,
    )
    .bind(&key)
    .fetch_one(db)
    .await
    .map_err(dberr)?;
    if grants.0 > 0 {
        let why = "发放账是余量与每人上限的唯一真相，\
                   删物品会连账一起清掉 —— 请改为停用";
        return Err(DomainError::Validation(format!(
            "「{name}」已发放过 {} 件（{} 条账）。{why}",
            grants.1, grants.0
        )));
    }

    let deleted = sqlx::query("DELETE FROM arcade_items WHERE key = $1")
        .bind(&key)
        .execute(db)
        .await
        .map_err(dberr)?
        .rows_affected();
    state
        .repo
        .audit(Some(auth.id), "arcade.item.delete", None)
        .await;
    Ok(ok(json!({ "key": key, "deleted": deleted })))
}
