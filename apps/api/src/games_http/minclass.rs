//! 逐游戏等级门槛（0490）：site_settings.games_minclass_<key>。
//!
//! 九键 classlevel（迁移 0490，缺省 0 = 不限），API 硬闸在每款游戏的
//! 入口端点；前端大厅卡片只按 /games 总览的 minclass 清单置灰——
//! 这里才是执法点（防低级号直接调 API 刷魔力）。
//! 读失败按 0 放行（与经济键的保守值纪律相反：门槛是准入不是安全阀，
//! DB 抖一下把全站玩家挡在门外比放行更伤）。gacha_http/draw 与 me
//! 也走同一把尺（键 games_minclass_gacha）。

use actix_web::web;

use crate::errors::{DomainError, DomainResult};
use crate::state::AppState;

/// 门槛读数（0-89 钳位：90+ 是职务档，游戏不锁职级）
pub(crate) async fn minclass_of(
    state: &web::Data<std::sync::Arc<AppState>>,
    game: &str,
) -> i32 {
    sqlx::query_scalar(
        "SELECT COALESCE((SELECT value FROM site_settings \
         WHERE name = 'games_minclass_' || $1)::int, 0)",
    )
    .bind(game)
    .fetch_one(&state.repo.db)
    .await
    .unwrap_or(0)
    .clamp(0, 89)
}

/// API 硬闸：低于该游戏等级门槛的用户拒绝并给出解锁等级。
pub(crate) async fn check_minclass(
    state: &web::Data<std::sync::Arc<AppState>>,
    game: &str,
    auth_class: i32,
) -> Result<(), DomainError> {
    let need = minclass_of(state, game).await;
    if auth_class < need {
        return Err(DomainError::Validation(format!("该玩法 {need} 级解锁")));
    }
    Ok(())
}
