//! 农场土地阶梯（读侧）：地块持有状态、报价与播种关闸。
//!
//! 机制与算式都在 `games::farm_land`，参数在 site_settings（迁移 0260），这里
//! 只做「读行表 + 派生投影」。买地/升级这两个动账动作在 `farm_land_write.rs`。
//!
//! 这条阶梯是**确定性魔力沉没口**：只出不进、零退款，不碰 EV —— 所以它不过
//! EV 闸，但**必须**过参数闸（配错的阶梯会把地块白送出去，那是另一种增发）。

use actix_web::web;

use crate::errors::{DomainError, DomainResult};
use crate::games::{
    self, land_config, land_price, next_purchasable_slot, speed_permille,
    upgrade_price, LandConfig, MAX_LEVEL,
};
use crate::state::AppState;

use super::helpers::eco_i64;

/// 报错统一成能读懂的一句话（配置面与写侧共用这一份措辞）
pub(super) fn land_err(e: impl std::fmt::Display) -> DomainError {
    DomainError::Validation(e.to_string())
}

/// 我的土地状态：站长参数 + 已买块数 + 每块等级。
///
/// 「没行的地块就是 1 级」是这里唯一的派生默认：免费地块不播种持有行
///（见迁移 0260 的 CHECK），所以这张表只记**买来的**与**升过级的**地。
pub(super) struct LandState {
    cfg: LandConfig,
    free: i32,
    purchased: i32,
    levels: Vec<(i32, i32)>,
}

impl LandState {
    /// 免费地块数（`games::FARM_PLOTS`）：1..=free 不需要买
    pub(super) fn free(&self) -> i32 {
        self.free
    }

    /// 已买块数 —— 阶梯的档数就按它走
    pub(super) fn purchased(&self) -> i32 {
        self.purchased
    }

    pub(super) fn cap(&self) -> i32 {
        self.cfg.max_plots
    }

    /// 已持有到的槽位数（免费 + 买来的；买来的按阶梯连续排，不会跳号）
    pub(super) fn owned(&self) -> i32 {
        self.free + self.purchased
    }

    pub(super) fn level(&self, slot: i32) -> i32 {
        self.levels
            .iter()
            .find(|(s, _)| *s == slot)
            .map(|(_, l)| *l)
            .unwrap_or(1)
    }

    /// 下一块地的报价（档数 = 已买块数）
    pub(super) fn land_quote(&self) -> i64 {
        land_price(self.cfg.land_base, self.cfg.land_ratio, self.purchased)
    }

    /// 从 `from` 级往上升一级的报价
    pub(super) fn up_quote(&self, from: i32) -> i64 {
        upgrade_price(self.cfg.up_base, self.cfg.up_ratio, from)
    }

    /// 播种关闸：槽位必须持有，返回成熟**分钟**数。
    /// 等级在这里、也只在这里影响结果 —— 它改的是时长，不是产量。
    fn plant_minutes(&self, slot: i32, grow_hours: i32) -> Option<i64> {
        if slot < 1 || slot > self.owned() {
            return None;
        }
        Some(games::grow_minutes(grow_hours, self.level(slot)))
    }

    /// 展示投影：前台地块格与后台参数卡读同一份，不各算一遍价
    fn projection(&self) -> serde_json::Value {
        let next = next_purchasable_slot(self.free, self.purchased, self.cap())
            .ok();
        let plots: Vec<serde_json::Value> = (1..=self.owned())
            .map(|slot| {
                let level = self.level(slot);
                let maxed = level >= MAX_LEVEL;
                serde_json::json!({
                    "slot": slot,
                    "level": level,
                    "bought": slot > self.free,
                    "speed_permille": speed_permille(level),
                    "maxed": maxed,
                    "next_upgrade_price": if maxed {
                        0
                    } else {
                        self.up_quote(level)
                    },
                })
            })
            .collect();
        serde_json::json!({
            "free": self.free,
            "cap": self.cap(),
            "purchased": self.purchased,
            "owned": self.owned(),
            "next_slot": next,
            "next_land_price": next.map(|_| self.land_quote()),
            "max_level": MAX_LEVEL,
            "plots": plots,
        })
    }
}

/// 读站长参数 + 我的持有行。参数不合法在这里就拒，不带着坏配置往下走。
pub(super) async fn load_land(
    state: &web::Data<std::sync::Arc<AppState>>,
    user_id: i64,
) -> DomainResult<LandState> {
    let cap = eco_i64(state, "farm_max_plots", games::DEFAULT_MAX_PLOTS).await;
    let land_base =
        eco_i64(state, "farm_land_base", games::DEFAULT_LAND_BASE).await;
    let land_ratio = eco_i64(
        state,
        "farm_land_ratio_permille",
        games::DEFAULT_LAND_RATIO,
    )
    .await;
    let up_base = eco_i64(state, "farm_up_base", games::DEFAULT_UP_BASE).await;
    let up_ratio =
        eco_i64(state, "farm_up_ratio_permille", games::DEFAULT_UP_RATIO).await;
    let cfg = land_config(cap, land_base, land_ratio, up_base, up_ratio)
        .map_err(|e| {
            land_err(format!("农场土地参数不合法，已拒绝下单：{e}"))
        })?;
    let rows: Vec<(i32, i32)> = sqlx::query_as(
        "SELECT slot, level FROM farm_land WHERE user_id = $1 ORDER BY slot",
    )
    .bind(user_id)
    .fetch_all(&state.repo.db)
    .await
    .map_err(|e| DomainError::Internal(e.into()))?;
    let free = games::FARM_PLOTS;
    // 「已买块数」只数免费 allotment 之外的行：万一有人写出过一行免费地的
    // level=1 记录，它也不该让阶梯打折（列约束同样拦着这件事）
    let purchased = rows.iter().filter(|(s, _)| *s > free).count() as i32;
    Ok(LandState {
        cfg,
        free,
        purchased,
        levels: rows,
    })
}

/// 播种前的地块关闸：返回成熟分钟数（等级只买周转，不改产量）。
/// `farm_actions::farm_plant` 用它取代过去写死的 1..=6 判定。
pub(super) async fn plant_guard(
    state: &web::Data<std::sync::Arc<AppState>>,
    user_id: i64,
    slot: i32,
    grow_hours: i32,
) -> DomainResult<i64> {
    let land = load_land(state, user_id).await?;
    land.plant_minutes(slot, grow_hours).ok_or_else(|| {
        land_err(format!(
            "第 {slot} 号地块还不是你的：现在能种的是 1..{}，更多地块要先买",
            land.owned()
        ))
    })
}

/// 土地状态的只读投影（farm_overview 用它，两个动作的响应各自另说）
pub(super) async fn land_projection(
    state: &web::Data<std::sync::Arc<AppState>>,
    user_id: i64,
) -> DomainResult<serde_json::Value> {
    Ok(load_land(state, user_id).await?.projection())
}
