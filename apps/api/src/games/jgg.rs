//! 九宫格抽奖（jgg 口径）：期望回报 0.725，庄家优势 27.5%。

use rand::Rng;

/// 奖池档位（管理端可配的简化常量版）：期望回报 = 0.725，庄家优势 27.5%
/// （修复前 50x/100x 权重过高致 EV=3.43 持续增发；按下表权重×赔率精确复算）
pub struct JggPrize {
    pub label: &'static str,
    pub weight: u32, // 权重（总 1000）
    pub payout: i64, // 相对票价倍数（10 = 10x）
}

pub const JGG_PRIZES: [JggPrize; 8] = [
    JggPrize {
        label: "谢谢参与",
        weight: 731,
        payout: 0,
    },
    JggPrize {
        label: "再来一次",
        weight: 120,
        payout: 1,
    },
    JggPrize {
        label: "2x 魔力",
        weight: 60,
        payout: 2,
    },
    JggPrize {
        label: "3x 魔力",
        weight: 50,
        payout: 3,
    },
    JggPrize {
        label: "5x 魔力",
        weight: 25,
        payout: 5,
    },
    JggPrize {
        label: "10x 魔力",
        weight: 10,
        payout: 10,
    },
    JggPrize {
        label: "20x 魔力",
        weight: 3,
        payout: 20,
    },
    JggPrize {
        label: "50x 魔力",
        weight: 1,
        payout: 50,
    },
];

pub const JGG_TICKET: i64 = 100;

pub struct JggDraw {
    pub index: usize, // 中奖格子 0..8（渲染九宫格）
    pub prize: &'static JggPrize,
}

pub fn jgg_draw() -> JggDraw {
    let total: u32 = JGG_PRIZES.iter().map(|p| p.weight).sum();
    let mut roll: u32 = rand::thread_rng().gen_range(0..total);
    for (i, p) in JGG_PRIZES.iter().enumerate() {
        if roll < p.weight {
            return JggDraw { index: i, prize: p };
        }
        roll -= p.weight;
    }
    unreachable!("weights sum to total")
}
