//! 商店目录端点 + 反通胀阀门（从 shop.rs 拆出，300 行门禁）。

use actix_web::{get, web, HttpRequest, Responder};

use crate::dto::ok;
use crate::errors::{DomainError, DomainResult};
use crate::state::AppState;

#[derive(sqlx::FromRow, serde::Serialize)]
struct ShopItem {
    id: i64,
    name: String,
    kind: String,
    price: i64,
    /// 0207：前端要按 config 判定可否选数量（stackable）、装扮候选（frame_id）
    config: serde_json::Value,
    /// 装扮类商品：当前用户是否已拥有（0207b 商店「已拥有」态）
    #[sqlx(default)]
    owned: bool,
    /// 剩余库存（商城审计 P2）：null=不限量；0=售罄（前端据此置灰按钮，
    /// 不再让用户点了购买才收到「已售罄」）
    #[sqlx(default)]
    stock_left: Option<i64>,
}

/// 反通胀阀门（C5，UNIT3D max-buffer-to-buy-upload 同款）：上传量类商品在
/// 用户缓冲量（上传-下载）已达 economy_max_buffer_gb 时拒购；0/缺键=不限。
/// 只拦「花钱买上传量」，券/卡牌/装扮不受影响。
pub(super) async fn buffer_cap_check(
    db: &sqlx::PgPool,
    user_id: i64,
) -> DomainResult<()> {
    let cap_gb: f64 = sqlx::query_scalar(
        "SELECT COALESCE((SELECT value FROM site_settings \
         WHERE name = 'economy_max_buffer_gb')::float8, 0)",
    )
    .fetch_one(db)
    .await
    .unwrap_or(0.0);
    if cap_gb <= 0.0 {
        return Ok(());
    }
    let (up, down): (i64, i64) =
        sqlx::query_as("SELECT uploaded, downloaded FROM users WHERE id = $1")
            .bind(user_id)
            .fetch_one(db)
            .await
            .map_err(|e| DomainError::Internal(e.into()))?;
    let buffer_gb = (up - down).max(0) as f64 / 1024.0 / 1024.0 / 1024.0;
    if buffer_gb >= cap_gb {
        return Err(DomainError::Validation(format!(
            "你的缓冲量已达 {buffer_gb:.0}GB（上限 {cap_gb:.0}GB），\
             暂不能继续购买上传量类商品"
        )));
    }
    Ok(())
}

/// 目录里的「价」必须与下单扣款的价是同一个式子。
/// 0383 起单价可按已售量递增（`price_step`/`price_max`/`price_anchor_qty`），
/// 但目录只回 `shop_items.price` 这一列死值 ⇒ 站方一旦给某个商品配上递增档，
/// 商店页写 5000、点下去扣 8000：会员先在价格上看错，再在余额上看到。
/// 单价本体只有一处（`domain::shop::price_with_step`，购买口 shop.rs 用的就是它），
/// 这里把它同样用到目录上。配置漏填（开了 step 没填 max）照实报错，
/// 不兜底回显一个假价——那跟不配档位没区别，而且更糟（价是错的）。
fn eff_price(
    base: i64,
    sold: i64,
    step: Option<i64>,
    max: Option<i64>,
    anchor: i32,
) -> DomainResult<i64> {
    crate::domain::shop::price_with_step(base, sold, step, max, anchor)
}

/// 目录取数行：比 `ShopItem` 多带档位四列（`owned` 由 $1 为 NULL 时自然变假）
#[derive(sqlx::FromRow)]
struct ItemRow {
    id: i64,
    name: String,
    kind: String,
    price: i64,
    config: serde_json::Value,
    owned: bool,
    stock_left: Option<i64>,
    price_step: Option<i64>,
    price_max: Option<i64>,
    price_anchor_qty: i32,
    sold: i64,
}

#[get("/shop/items")]
pub(super) async fn shop_items(
    req: HttpRequest,
    state: web::Data<std::sync::Arc<AppState>>,
) -> DomainResult<impl Responder> {
    // 匿名可看目录：$1 绑 NULL 时 EXISTS 恒假 ⇒ owned 全员 false，
    // 不必再按 kind 拆两条 UNION（那条拆分只为给装扮类算 owned，
    // 而 user_dressups 里本来只会有装扮类的行）
    let uid: Option<i64> = crate::http::require_auth(&req, &state)
        .await
        .ok()
        .map(|a| a.id);
    let rows: Vec<ItemRow> = sqlx::query_as(
        "SELECT si.id, si.name, si.kind, si.price, si.config, \
         EXISTS(SELECT 1 FROM user_dressups ud \
         WHERE ud.user_id = $1 AND ud.item_id = si.id) AS owned, \
         (si.stock_quota - si.stock_used) AS stock_left, \
         si.price_step, si.price_max, si.price_anchor_qty, \
         COALESCE(si.stock_used, 0) AS sold \
         FROM shop_items si WHERE si.active = true ORDER BY si.price",
    )
    .bind(uid)
    .fetch_all(&state.repo.db)
    .await
    .map_err(|e| DomainError::Internal(e.into()))?;
    let mut items: Vec<ShopItem> = Vec::with_capacity(rows.len());
    for r in rows {
        items.push(ShopItem {
            id: r.id,
            name: r.name,
            kind: r.kind,
            price: eff_price(
                r.price,
                r.sold,
                r.price_step,
                r.price_max,
                r.price_anchor_qty,
            )?,
            config: r.config,
            owned: r.owned,
            stock_left: r.stock_left,
        });
    }
    Ok(ok(items))
}
