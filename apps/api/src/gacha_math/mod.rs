//! 抽卡数学层 —— 样张 `.workbuddy/gacha-shared.js` 的 Rust 镜像
//! （方案《抽卡玩法落地方案-2026-09-27》§3「必须先抽出来的一层」）。
//!
//! TS 侧在 `packages/gacha-math/src/index.ts`（前端公示用），本实现服务守卫与
//! 运行时预检（G31-B：抽取前算 `ratio_worst`，>100% 直接 409——站长直连改库也要拦）。
//! 两侧共用同一份向量 `gacha_math_vectors.json`（由样张 oracle 生成），用例在
//! `tests.rs` 逐条镜像 `apps/web/tests/gacha-math.test.ts`。
//! **改任何一行算术都必须同步另一侧实现并用样张重生成向量**，否则「四处同源」
//! 一进真代码就破。
//!
//! 算术与样张逐位对齐：f64 运算顺序、`x || 0`/`x || 1` 的归一化语义
//! （0 与缺失同待遇，见各处注释）都照抄。JS 的 `Math.max/min` 在 NaN 上与
//! Rust `f64::max/min` 行为不同，但输入经 serde 反序列化不可能出现 NaN。

#![allow(dead_code)] // G31-B 接线前仅镜像用例使用；先立数学，后立调用方

use serde::Deserialize;

mod composite;
mod economics;
mod simulate;
#[cfg(test)]
mod tests;

pub use composite::{composite, CompositeResult};
pub use economics::{economics, EconomicsResult, SvFrom};
pub use simulate::{simulate, SimulateResult};

/// 「出金」= gold_rank ≥ 3（SSR 及以上）；真实实现以 gacha_rarities 行表为准（0226）。
pub const GOLD_MIN_RANK: f64 = 3.0;

/// 样张的档位表；rank 只服务 `is_gold`，颜色/星数等视觉 token 不进数学层。
fn rank_of(r: &str) -> f64 {
    match r {
        "R" => 1.0,
        "SR" => 2.0,
        "SSR" => 3.0,
        "UR" => 4.0,
        "LR" => 5.0,
        _ => 0.0,
    }
}

pub fn is_gold(r: &str) -> bool {
    rank_of(r) >= GOLD_MIN_RANK
}

/// 奖池行。碎片/谢谢惠顾不是稀有度，`r` 用 SHARD/MISS 占位。
/// 缺失字段按 0 处理（= 样张 `+x.f || 0`）；`lv_max` 例外，见 `economics`。
#[derive(Clone, Deserialize, Default)]
#[serde(default)]
pub struct RateRow {
    pub r: String,
    #[serde(rename = "type")]
    pub kind: String, // "card" | "shard" | "miss"
    /// 表定权重；综合概率由保底 DP 派生（`composite`），守卫与公示必须用后者。
    pub w: f64,
    /// 卡的价值。
    pub value: f64,
    /// 碎片档：一次产出数量。
    pub shards: f64,
    /// 卡档：满图鉴后重复卡的折算价。
    pub dupe: f64,
    /// 卡档：合成所需碎片数 —— 碎片单价的倒推来源之一。
    pub synth: f64,
    pub lv_max: f64,
    pub lv_step: f64,
    pub lv_gain: f64,
}

#[derive(Clone, Deserialize, Default)]
#[serde(rename_all = "camelCase", default)]
pub struct PityConfig {
    pub soft: f64,
    /// 硬保底（必出金）；≤1 按 1。
    pub hard: f64,
    /// 每抽抬升的百分点（把概率从非金档按比例搬到金档）。
    pub ramp: f64,
    /// UP 卡在金档内的目标占比；缺失按 1（样张 `== null ? 1`）。
    pub up_ratio: Option<f64>,
    /// 硬保底给的金是否必中 UP。
    pub guarantee_up: bool,
}

#[derive(Clone, Deserialize, Default)]
#[serde(default)]
pub struct PoolConfig {
    pub rates: Vec<RateRow>,
    pub pity: PityConfig,
}

/// 可复现 PRNG（mulberry32）：同种子必同结果，seed 落库以便事后复核。
/// 与 TS `rng` 逐位同流（纯 u32 包绕运算，seed 按 `|0` 截断）。
pub fn rng(seed: i64) -> impl FnMut() -> f64 {
    let mut a = seed as i32 as u32;
    move || {
        a = a.wrapping_add(0x6D2B_79F5);
        let mut t = (a ^ (a >> 15)).wrapping_mul(a | 1);
        t = t.wrapping_add((t ^ (t >> 7)).wrapping_mul(61 | t)) ^ t;
        f64::from(t ^ (t >> 14)) / 4294967296.0
    }
}
