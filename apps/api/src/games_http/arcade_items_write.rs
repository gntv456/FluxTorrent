//! 物品目录保存（后台写侧）。
//!
//! 从 arcade_admin_write 拆出：两个写端点的**校验面不同** ——
//! 改一个物品的 anchor 会连带改掉所有引用它的池的 EV，
//! 必须跨池回查并整体回滚。这条逻辑独立出来才看得清，
//! 也不会把两个端点的行数叠在一起撞 300 上限。

use actix_web::{post, web, HttpRequest, HttpResponse};
use serde::Deserialize;
use serde_json::json;

use crate::authz;
use crate::dto::ok;
use crate::errors::{DomainError, DomainResult};
use crate::games;
use crate::http::require_auth;
use crate::state::AppState;

use super::item_use::{check_use_shape, use_kind_or_collect};
use super::pool::dberr;

/// 目录项保存请求。**带默认值的字段一律是 `Option`**：
/// 一个「只想改名字」的请求如果不带 `unlimited`，按缺省 `true` 落库就等于
/// 悄悄把一件限量物品改成不限量、把 `use_kind` 改回 collect、把图标清空——
/// 配置面自己把站长没打算改的东西改掉了，而且读不出区别。
/// 规则：**缺字段 = 保持库里现值**，显式传值（包括传 `""` 清空图标）才覆盖。
#[derive(Deserialize)]
pub(super) struct ItemSaveReq {
    pub key: String,
    pub name: String,
    pub kind: String,
    pub anchor: i64,
    pub anchor_src: String,
    #[serde(default)]
    pub unlimited: Option<bool>,
    #[serde(default)]
    pub stock: Option<i64>,
    #[serde(default)]
    pub per_user: Option<i32>,
    #[serde(default)]
    pub icon: Option<String>,
    #[serde(default)]
    pub enabled: Option<bool>,
    /// 用途：collect 收藏 | spark 兑现魔力 | sku 走商店生效链（见 item_use.rs）
    #[serde(default)]
    pub use_kind: Option<String>,
    /// sku 时填绑定的 shop_items.id
    #[serde(default)]
    pub use_ref: Option<String>,
}

/// 一件新物品从零入库时的初值（目录里还没有这一行时）
const NEW_ITEM_DEFAULTS: (bool, i64, i32, &str, bool, &str, &str) =
    (true, 0, 1, "", true, "collect", "");

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
    if let Some(k) = b.use_kind.as_deref().filter(|s| !s.is_empty()) {
        if !matches!(k, "collect" | "spark" | "sku") {
            return Err(DomainError::Validation(format!(
                "use_kind 只能是 collect | spark | sku，收到「{k}」"
            )));
        }
    }
    // 经济类物品必须报得出正折算价，否则「零负债」是假的
    if b.kind == "economic" && b.anchor <= 0 {
        return Err(DomainError::Validation(format!(
            "economic 物品「{}」anchor 必须为正：它按定义就是站点负债",
            b.key
        )));
    }

    // 先读现值：所有「请求里没带」的字段都保持原样，校验也按生效后的值判
    let prev = sqlx::query_as::<
        _,
        (bool, i64, i32, String, bool, String, String),
    >(
        "SELECT unlimited, stock, per_user, icon, enabled, \
                use_kind, use_ref \
           FROM arcade_items WHERE key = $1",
    )
    .bind(&b.key)
    .fetch_optional(db)
    .await
    .map_err(dberr)?;
    let dflt = NEW_ITEM_DEFAULTS;
    let (p_unl, p_stock, p_pu, p_icon, p_en, p_uk, p_ur) =
        prev.unwrap_or((
            dflt.0, dflt.1, dflt.2, dflt.3.to_string(), dflt.4,
            dflt.5.to_string(), dflt.6.to_string(),
        ));
    let unlimited = b.unlimited.unwrap_or(p_unl);
    let stock = b.stock.unwrap_or(p_stock);
    let per_user = b.per_user.unwrap_or(p_pu);
    let icon = b.icon.unwrap_or(p_icon);
    let enabled = b.enabled.unwrap_or(p_en);
    let use_kind = match b.use_kind.as_deref().filter(|s| !s.is_empty()) {
        Some(k) => use_kind_or_collect(k).to_string(),
        None => p_uk,
    };
    let use_ref = b.use_ref.unwrap_or(p_ur);

    if per_user <= 0 {
        return Err(DomainError::Validation("每人上限必须为正".into()));
    }
    if !unlimited && stock < 0 {
        return Err(DomainError::Validation("限量物品 stock 不能为负".into()));
    }
    // 停用一件正被确定侧奖励引用的物品 = 那条奖励当场领不出去（确定侧没有
    // 「打折回落」可用）。写侧先拒，别让面板自己制造出门禁才照得出的坏状态。
    if !enabled {
        let n: i64 = sqlx::query_scalar(
            "SELECT count(*)::bigint FROM ( \
               SELECT item_key FROM arcade_quests WHERE enabled \
               UNION ALL \
               SELECT item_key FROM arcade_milestones WHERE enabled \
             ) r WHERE r.item_key = $1",
        )
        .bind(&b.key)
        .fetch_one(db)
        .await
        .map_err(dberr)?;
        if n > 0 {
            return Err(DomainError::Validation(format!(
                "「{}」仍被 {n} 条启用中的确定侧奖励引用：\
                 先把那些奖励改掉再停用",
                b.name
            )));
        }
    }
    // 用途侧的形制闸（详见 item_use.rs）：绑了商店 SKU 就必须兑得出等价的东西
    check_use_shape(db, &use_kind, &use_ref, b.anchor, &b.name).await?;

    let mut tx = db.begin().await.map_err(dberr)?;
    sqlx::query(
        r#"
        INSERT INTO arcade_items
            (key, name, kind, anchor, anchor_src,
             unlimited, stock, per_user, icon, enabled, use_kind, use_ref)
        VALUES ($1, $2, $3, $4, $5, $6, $7, $8, $9, $10, $11, $12)
        ON CONFLICT (key) DO UPDATE SET
            name = EXCLUDED.name, kind = EXCLUDED.kind,
            anchor = EXCLUDED.anchor, anchor_src = EXCLUDED.anchor_src,
            unlimited = EXCLUDED.unlimited, stock = EXCLUDED.stock,
            per_user = EXCLUDED.per_user, icon = EXCLUDED.icon,
            enabled = EXCLUDED.enabled, use_kind = EXCLUDED.use_kind,
            use_ref = EXCLUDED.use_ref, updated_at = now()
    "#,
    )
    .bind(&b.key)
    .bind(&b.name)
    .bind(&b.kind)
    .bind(b.anchor)
    .bind(&b.anchor_src)
    .bind(unlimited)
    .bind(stock)
    .bind(per_user)
    .bind(&icon)
    .bind(enabled)
    .bind(&use_kind)
    .bind(use_ref.trim())
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
            SELECT e.label, e.weight, e.mult_permille, e.kind,
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
            .map(|(label, weight, mult_permille, kind, ik, qty, anchor)| {
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
                        games::EntryKind::Magic { mult_permille }
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
    Ok(ok(json!({
        "key": b.key,
        "anchor": b.anchor,
        "kind": b.kind,
        "use_kind": use_kind,
        "use_ref": use_ref.trim(),
    })))
}

