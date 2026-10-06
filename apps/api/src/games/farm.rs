//! 农场：确定性市场价窗口 + 买卖两侧错开因子（防低买高卖套利）。
//!
//! 作物全部为通用幻想系命名（四叶草/星尘豆/云端瓜/月华参/日冕稻），**不绑定任何站型特色**
//! （教育站、影音站、音乐站都用同一套），站长可在后台改名。
//! 产量按「种子价 × 0.75」标定：收获期望 = 0.75 × (1 + 20% 双倍) = 0.90 < 1，回收口径。
//! 收获之上还有一档站长可配的**彩蛋奖池**（`arcade_pools` game='farm'，见 farm_egg.rs）：
//! 它只能吃 0.90 剩下的 0.10，由 `validate_farm` 把关。

use rand::Rng;

use super::jgg::{
    ev_strictly_below, pool_ev, validate_shape, PoolEntry, PoolError, MULT_UNIT,
};

/// 作物表标定与双倍率都写成分数：产量 = 种子价 × 3/4，20% 概率双倍。
/// 基础回收率是这两者相乘（18/20 = 0.90），不是第三个数 —— 改标定等于改经济口径，
/// 而 `validate_crop`、迁移 0253 的 CHECK、彩蛋池的余量全都以这条乘积为准。
/// 用分数而不是小数，是因为闸门必须做**精确**比较（见 `ev_strictly_below`）。
const YIELD_NUM: i64 = 3;
const YIELD_DEN: i64 = 4;
const DOUBLE_NUM: i64 = 1;
const DOUBLE_DEN: i64 = 5;

/// 基础回收率的分子/分母：(3 × (5+1)) / (4 × 5) = 18/20
pub const BASE_EV_NUM: i64 = YIELD_NUM * (DOUBLE_DEN + DOUBLE_NUM);
pub const BASE_EV_DEN: i64 = YIELD_DEN * DOUBLE_DEN;

/// 农场收获侧的**基础**回收率上限 0.90 —— 只用于展示与报账，
/// 判据一律走上面的分数。
pub const BASE_EV: f64 = BASE_EV_NUM as f64 / BASE_EV_DEN as f64;

/// 成熟时长区间（小时）：1 小时到 30 天
pub const GROW_HOURS_MIN: i32 = 1;
pub const GROW_HOURS_MAX: i32 = 720;

#[derive(Debug, Clone, PartialEq)]
pub enum CropError {
    BadPrice(i64),
    BadYield(i64),
    BadGrow(i32),
    /// 突破标定：收获期望高过 BASE_EV，农场就不再是回收口
    OverCalibration {
        ev: f64,
        cap: f64,
    },
}

impl std::fmt::Display for CropError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            CropError::BadPrice(p) => {
                write!(f, "种子价需在 1 ~ {} 之间，实为 {p}", i32::MAX)
            }
            CropError::BadYield(y) => {
                write!(f, "基准产量需在 1 ~ {} 之间，实为 {y}", i32::MAX)
            }
            CropError::BadGrow(h) => write!(
                f,
                "成熟时长需在 {}–{} 小时之间，实为 {h}",
                GROW_HOURS_MIN, GROW_HOURS_MAX
            ),
            CropError::OverCalibration { ev, cap } => write!(
                f,
                "这一档收获期望 {ev:.3} 已超过作物表标定的上限 {cap:.2}：\
                 产量要按「种子价 × 0.75」标定，否则农场在增发而不是回收，\
                 彩蛋池那 0.10 的余量也算不住了"
            ),
        }
    }
}

/// 一株作物的收获期望：产量 ÷ 种子价 × (1 + 双倍率)。
/// 0 种子价不返回 0 而是正无穷 —— 「白送的种子」是最坏情况，不是没情况。
pub fn crop_expected_value(seed_price: i64, base_yield: i64) -> f64 {
    if seed_price <= 0 {
        return f64::INFINITY;
    }
    (base_yield as f64 / seed_price as f64)
        * (1.0 + DOUBLE_NUM as f64 / DOUBLE_DEN as f64)
}

/// 作物档位关闸（写侧与迁移 0253 的 CHECK 同源）：正数、成熟时长在区间内，
/// 且**不得突破标定**。判据用整数式 `产量 × 4 <= 种子价 × 3`（与 CHECK 一字不差），
/// 不引入浮点容差。
pub fn validate_crop(
    seed_price: i64,
    base_yield: i64,
    grow_hours: i32,
) -> Result<(), CropError> {
    // 上限是列宽（INT4）：越界必须由这里点名，不能让调用方 `as i32` 绕成负数
    let fits = |v: i64| (1..=i64::from(i32::MAX)).contains(&v);
    if !fits(seed_price) {
        return Err(CropError::BadPrice(seed_price));
    }
    if !fits(base_yield) {
        return Err(CropError::BadYield(base_yield));
    }
    if grow_hours < GROW_HOURS_MIN || grow_hours > GROW_HOURS_MAX {
        return Err(CropError::BadGrow(grow_hours));
    }
    if base_yield * YIELD_DEN > seed_price * YIELD_NUM {
        return Err(CropError::OverCalibration {
            ev: crop_expected_value(seed_price, base_yield),
            cap: BASE_EV,
        });
    }
    Ok(())
}

/// 每人地块数：写侧校验、地块投影、总览三处读同一个数（前台按 API 给的 slots 画）。
pub const PLOTS: i32 = 6;

/// 农场额外奖池的总口径：确定性收获（BASE_EV）+ 奖池那一注。
/// 单位 unit 取**最便宜作物的种子价**：额外档按种子价倍数派彩，
/// 而收获本身也按同一倍数标定，所以比值对每一株作物都一样 ——
/// 最便宜那一株就是最坏情况。
pub fn farm_total_ev(entries: &[PoolEntry], unit: i64) -> f64 {
    BASE_EV + pool_ev(entries, unit)
}

/// 农场收获彩蛋档的派彩：这一株的种子价 × 千分倍率。
/// 与 `PoolEntry::value()` 同一条向下取整口径（对站点有利的取整侧）。
pub fn egg_pay(seed_price: i64, mult_permille: i64) -> i64 {
    seed_price.saturating_mul(mult_permille) / MULT_UNIT
}

/// 农场奖池关闸：额外那一注不能把总回收推过 1。
/// 0.90 的余量只有 0.10 —— 这就是「农场发物品必须限量」的硬账。
/// 结构项与另三个玩法共用 `validate_shape`，只有 EV 判据不同。
pub fn validate_farm(
    entries: &[PoolEntry],
    unit: i64,
) -> Result<(), PoolError> {
    validate_shape(entries, unit)?;
    // 「0.90 + 彩蛋 < 1」等价于「彩蛋 < 1/10」，两侧都用整数判：
    // 浮点会把恰好压在余量上的那张池算成 0.9999999999999999 而放过去
    if !ev_strictly_below(entries, unit, BASE_EV_DEN - BASE_EV_NUM, BASE_EV_DEN)
    {
        return Err(PoolError::ExpectedValueNotBelowOne(farm_total_ev(
            entries, unit,
        )));
    }
    Ok(())
}

/// 市场价波动窗口：默认 4 小时（0109 games farm_market_window_hours 可调，worker
/// 与 API 各自读取；窗口跨小时数变化只影响新窗口起点，历史价不重算）
pub fn market_window_hours() -> i64 {
    // 独立常量读取：农场域无 AppState 上下文（纯函数），经 OnceLock 缓存 env/缺省 4h。
    // 设置键在 DB，纯函数不便 async 读——0109 键以「缺省=代码默认」语义存在，
    // 消费方（farm_jobs/games_http）落库前读 site_settings 后传参进来；此函数保留
    // 兜底口径（窗口必须是 3600 整数倍，最小 1h）。
    4
}

/// 市场价波动窗口起点（0/4/8/12/16/20 点口径；hours 参数来自设置键）
pub fn market_window_start_with(ts: i64, hours: i64) -> i64 {
    let window = hours.max(1) * 3600;
    ts - (ts % window)
}

/// 市场价波动窗口：每日 0/4/8/12/16/20 点刷新一次（§M24 规则要点）
pub fn market_window_start(ts: i64) -> i64 {
    market_window_start_with(ts, market_window_hours())
}

/// 确定性市场价：基准价 ±50% 波动，同一价格窗口内稳定。
/// 用窗口起点做种子（不是真随机），保证全站所有用户同一窗口看到同一价格。
/// 2026-10-06 安全审计 P1-1 后本函数**只用于行情展示**——实际买种扣费与
/// 收获入账都改用 `roll_market_factor` 在请求内真随机掷出（见下）。
pub fn market_price(seed_price: i64, window_start: i64) -> i64 {
    let unit = market_unit(window_start, 0);
    let factor = 50 + (unit % 101); // 50..150
    (seed_price * factor as i64) / 100
}

/// 市场因子：请求内服务端真随机掷出（2026-10-06 安全审计 P1-1）。
///
/// 旧实现买/卖两侧同为「窗口起点纯哈希」（salt 错开）——两因子虽独立但都
/// **可离线预计算**：作弊者可筛「低价窗口买入 × 高价窗口收获」确定性套利
/// （约 35-40% 组合 ROI>1）；只随机化收获侧也不够——专挑最低价买窗仍有
/// 0.75×100/50 = 1.5 的期望 ROI（回归测试
/// `random_buy_and_sell_factors_yield_house_edge` 锁此教训）。
/// 因此**买种扣费与收获入账都在各自请求内**用本函数掷因子（thread_rng，
/// CSPRNG），窗口行情价（market_price）降级为纯展示参考；两侧期望因子
/// 均 ×1.0，作物回收率 0.90 < 1 的标定不受影响。
pub fn roll_market_factor() -> i64 {
    50 + rand::thread_rng().gen_range(0..101i64)
}

/// 按给定市场因子折算价格（roll_market_factor 的配套）。
pub fn apply_market_factor(base: i64, factor: i64) -> i64 {
    (base * factor) / 100
}

/// xorshift64* 窗口哈希 → 高 31 位；salt 区分买/卖两侧因子
fn market_unit(window_start: i64, salt: u64) -> u64 {
    let mut x = (window_start as u64 ^ 0x9E3779B97F4A7C15).wrapping_add(salt);
    x ^= x >> 12;
    x ^= x << 25;
    x ^= x >> 27;
    x.wrapping_mul(0x2545F4914F6CDD1D) >> 33
}

/// 双倍收获（真随机，概率就是 `DOUBLE_CHANCE` —— 与标定乘积同一份来源）
pub fn roll_double() -> bool {
    rand::thread_rng().gen_range(0..DOUBLE_DEN) < DOUBLE_NUM
}
