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

#[get("/shop/items")]
pub(super) async fn shop_items(
    req: HttpRequest,
    state: web::Data<std::sync::Arc<AppState>>,
) -> DomainResult<impl Responder> {
    // 匿名可看目录（owned=false 全员）；登录后附拥有态
    let uid: Option<i64> = crate::http::require_auth(&req, &state)
        .await
        .ok()
        .map(|a| a.id);
    let items: Vec<ShopItem> = match uid {
        Some(uid) => {
            let rows: Vec<(
                i64,
                String,
                String,
                i64,
                serde_json::Value,
                bool,
                Option<i64>,
            )> = sqlx::query_as(
                "SELECT si.id, si.name, si.kind, si.price, si.config, \
                 EXISTS(SELECT 1 FROM user_dressups ud \
                 WHERE ud.user_id = $1 AND ud.item_id = si.id) AS owned, \
                 (si.stock_quota - si.stock_used) AS stock_left \
                 FROM shop_items si WHERE si.active = true \
                 AND si.kind IN ('avatar_frame','animated_avatar',\
                 'rainbow_id','rainbow_name') \
                 UNION ALL \
                 SELECT si.id, si.name, si.kind, si.price, si.config, FALSE, \
                 (si.stock_quota - si.stock_used) AS stock_left \
                 FROM shop_items si WHERE si.active = true \
                 AND si.kind NOT IN ('avatar_frame','animated_avatar',\
                 'rainbow_id','rainbow_name') \
                 ORDER BY price",
            )
            .bind(uid)
            .fetch_all(&state.repo.db)
            .await
            .map_err(|e| DomainError::Internal(e.into()))?;
            rows.into_iter()
                .map(
                    |(id, name, kind, price, config, owned, stock_left)| {
                        ShopItem {
                            id,
                            name,
                            kind,
                            price,
                            config,
                            owned,
                            stock_left,
                        }
                    },
                )
                .collect()
        }
        None => {
            let rows: Vec<(
                i64,
                String,
                String,
                i64,
                serde_json::Value,
                Option<i64>,
            )> = sqlx::query_as(
                "SELECT id, name, kind, price, config, \
                 (stock_quota - stock_used) AS stock_left \
                 FROM shop_items WHERE active = true ORDER BY price",
            )
            .fetch_all(&state.repo.db)
            .await
            .map_err(|e| DomainError::Internal(e.into()))?;
            rows.into_iter()
                .map(
                    |(id, name, kind, price, config, stock_left)| ShopItem {
                        id,
                        name,
                        kind,
                        price,
                        config,
                        owned: false,
                        stock_left,
                    },
                )
                .collect()
        }
    };
    Ok(ok(items))
}
