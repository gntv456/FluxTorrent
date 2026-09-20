//! 农场：确定性市场价窗口 + 买卖两侧错开因子（防低买高卖套利）。
//!
//! 作物全部为通用幻想系命名（四叶草/星尘豆/云端瓜/月华参/日冕稻），**不绑定任何站型特色**
//! （教育站、影音站、音乐站都用同一套），站长可在后台改名。
//! 产量按「种子价 × 0.75」标定：收获期望 = 0.75 × (1 + 20% 双倍) = 0.90 < 1，回收口径。

use rand::Rng;

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
/// 审计修复（P1 套利）：买种与收获此前共用同一确定性因子——低价窗（0.5x）买入、
/// 高价窗（1.5x）收获可稳定锁定利润。现买卖两侧使用错开的独立哈希（salt 不同 →
/// 因子近似独立），期望收益回到作物表本身的档位差（0011：80/100 … 4800/5000，
/// 全档微亏防刷），波动只剩赌运气，无确定性正 EV 策略。
pub fn market_price(seed_price: i64, window_start: i64) -> i64 {
    let unit = market_unit(window_start, 0);
    let factor = 50 + (unit % 101); // 50..150
    (seed_price * factor as i64) / 100
}

/// 收获侧市场价（与买种侧 salt 错开，消除「同因子低买高卖」套利）
pub fn harvest_market_price(base_yield: i64, window_start: i64) -> i64 {
    let unit = market_unit(window_start, 0x5DEECE66D);
    let factor = 50 + (unit % 101); // 50..150
    (base_yield * factor as i64) / 100
}

/// xorshift64* 窗口哈希 → 高 31 位；salt 区分买/卖两侧因子
fn market_unit(window_start: i64, salt: u64) -> u64 {
    let mut x = (window_start as u64 ^ 0x9E3779B97F4A7C15).wrapping_add(salt);
    x ^= x >> 12;
    x ^= x << 25;
    x ^= x >> 27;
    x.wrapping_mul(0x2545F4914F6CDD1D) >> 33
}

/// 20% 概率双倍收获（真随机）
pub fn roll_double() -> bool {
    rand::thread_rng().gen_range(0..100) < 20
}
