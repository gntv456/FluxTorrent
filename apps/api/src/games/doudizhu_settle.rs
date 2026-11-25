//! 斗地主结算与经济口径（从 doudizhu.rs 拆出：主文件贴 300 行门禁）。
//!
//! 倍数合成、抽水后派彩、EV 静态闸与派彩幂等键。规则识别在
//! [`crate::games::doudizhu`]，两者仅在结算点交汇。

use super::doudizhu::{MULTIPLIER_CAP, RAKE_PERMILLE_HARD_CAP};

/// 倍数：叫分底 × 2^炸弹数（王炸按两颗炸弹计）× 春天 ×2，封顶 8。
pub fn multiplier_of(call_base: i64, bombs: u32, spring: bool) -> i64 {
    let mut m = call_base.clamp(1, 3);
    m <<= bombs.min(3); // 2^bombs，bombs ≥3 已超封顶
    if spring {
        m <<= 1;
    }
    m.min(MULTIPLIER_CAP)
}

/// 抽水后倍率（千分比）：倍数 × (1000 − rake)，向下取整；rake 钳硬上限。
pub fn raked_mult_permille(mult: i64, rake_permille: i64) -> i64 {
    let rake = rake_permille.clamp(0, RAKE_PERMILLE_HARD_CAP);
    mult.saturating_mul(1000 - rake) / 1000
}

/// 结算口径（玩家视角，AI 房 = 玩家 vs 庄）：
/// - 开局扣 stake；赢家回款 = 本金 stake + 抽水后净收益。
/// - 净收益 = stake × share × multiplier × (1 − rake)，
///   其中 share 农民=1、地主=2（地主赢两家）。
/// - 玩家输：无回款（净 = −stake，开局已扣）。
///
/// 关键修正：旧版把本金 stake 一并计入抽水基数，导致
/// 「农民赢、倍数 1、抽水 10%」时回款 90 < 本金 100 —— 赢了反而亏 10。
/// 现在本金全额返还，只对净收益抽水，赢局 net 恒 ≥ 0。
/// 乘法链全部先乘后除，避免小倍数先除截断成 0。
pub fn settle_payout(
    role: Role,
    stake: i64,
    multiplier: i64,
    rake_permille: i64,
) -> i64 {
    let share = match role {
        Role::Farmer => 1,
        Role::Landlord => 2,
    };
    let rake = rake_permille.clamp(0, RAKE_PERMILLE_HARD_CAP);
    let win = stake
        .saturating_mul(share)
        .saturating_mul(multiplier)
        .saturating_mul(1000 - rake)
        / 1000;
    stake.saturating_add(win)
}

/// 玩家座位角色。
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Role {
    Landlord,
    Farmer,
}

/// 斗地主派彩幂等键：`game-doudizhu-win:<扣款幂等键>`。
///
/// 前缀约定同 `bs_room::win_idem`——`/games/rounds` 战绩口按
/// `'game-' || ref_type || '-win:' || <扣款键>` LEFT JOIN 派彩流水，
/// ref_type = `doudizhu`，两边任何一边改前缀战绩就断。
pub fn win_idem(bet_idem: &str) -> String {
    format!("game-doudizhu-win:{bet_idem}")
}

/// 静态胜率模型下的期望回报（回收口闸）：按玩家抽到地主的先验概率
/// （叫分胜出≈1/3）与两类角色的基准胜率（地主 32%、农民 34% 的简化
/// 先验——农民两家中只有一家与玩家同一阵营，另一家是对手，故玩家
/// 实际赢面按单农民口径 34% 计）加权。
///
/// 完整口径：以 stake 为单位，赢 = share×mult×(1−rake)，输 = −1；
/// 地主 share=2、农民 share=1，按先验概率加权。
/// EV(rake=100‰) ≈ 0.32 < 1，rake=0 ≈ 0.43 < 1——
/// 发牌随机+固定启发式 AI 下玩家没有技术优势通道。
pub fn doudizhu_expected_value(rake_permille: i64) -> f64 {
    let p_landlord = 1.0 / 3.0;
    let win_rate_landlord = 0.32;
    let win_rate_farmer = 0.34;
    let avg_mult = 2.5;
    let rake = (1000.0 - rake_permille as f64) / 1000.0;
    // 以 stake 为单位：赢 = share×mult×rake；输 = −1。
    // 地主 share=2、农民 share=1，按先验概率加权。
    let land_ev =
        win_rate_landlord * 2.0 * avg_mult * rake - (1.0 - win_rate_landlord);
    let farm_ev = win_rate_farmer * avg_mult * rake - (1.0 - win_rate_farmer);
    p_landlord * land_ev + (1.0 - p_landlord) * farm_ev
}
