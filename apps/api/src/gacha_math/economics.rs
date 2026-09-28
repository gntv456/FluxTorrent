//! 奖池含碎片/谢谢惠顾与卡牌等级后的经济闭环。
//! 关键（方案 §1.3）：碎片既是奖池产出、又是合成与升级的货币，其价值由
//! 「玩家能换到的最好东西」倒推 —— 合成价调低 ⇒ 碎片变值钱 ⇒ 奖池碎片行
//! 与重复卡折算同时升值，而卡片 value 一个字没改。守卫若只看卡片 value，
//! 就会被合成价这条后门绕过，所以口径必须取 max(新手, 毕业)。

use super::{composite, CompositeResult, PoolConfig};

/// 碎片单价的来源：合成（value/synth）或升级（value·lv_gain/升级总价）。
pub struct SvFrom {
    pub kind: String,
    pub r: String,
}

pub struct EconomicsResult {
    pub base: Vec<f64>,
    pub gold_base: f64,
    pub comp: Vec<f64>,
    pub gold_comp: f64,
    pub cycle: f64,
    pub hard_prob: f64,
    pub up_rate: f64,
    pub up_share: f64,
    pub ev_comp: f64,
    pub degenerate: bool,
    /// 碎片单价 = 逐卡取 max(value/synth, value·lv_gain/升级总价)。
    pub sv: f64,
    pub sv_from: Option<SvFrom>,
    /// 新手口径：新卡按全价。
    pub ev_new: f64,
    /// 毕业口径：卡全按重复折算 —— 满图鉴后它会反超新手口径。
    pub ev_end: f64,
    pub miss_rate: f64,
    pub shard_rate: f64,
    pub card_rate: f64,
    pub cost: f64,
    pub ratio_new: f64,
    pub ratio_end: f64,
    /// 守卫口径 = max(新手, 毕业)/cost；>1 即倒灌经济（合成价后门）。
    pub ratio_worst: f64,
}

pub fn economics(cfg: &PoolConfig, cost: f64) -> EconomicsResult {
    let co: CompositeResult = composite(cfg);
    let rates = &cfg.rates;
    let mut sv = 0.0f64;
    let mut sv_from: Option<SvFrom> = None;
    for x in rates.iter() {
        if x.kind != "card" {
            continue;
        }
        let v = x.value;
        if x.synth > 0.0 {
            let r = v / x.synth;
            if r > sv {
                sv = r;
                sv_from = Some(SvFrom {
                    kind: "synth".into(),
                    r: x.r.clone(),
                });
            }
        }
        // 样张 `(+x.lvMax || 1)`：缺失与 0 同按 1。
        let lv_max = if x.lv_max != 0.0 { x.lv_max } else { 1.0 };
        let lv_cost = x.lv_step * f64::max(0.0, lv_max - 1.0);
        if lv_cost > 0.0 && x.lv_gain > 0.0 {
            let r = v * x.lv_gain / lv_cost;
            if r > sv {
                sv = r;
                sv_from = Some(SvFrom {
                    kind: "level".into(),
                    r: x.r.clone(),
                });
            }
        }
    }
    let mut ev_new = 0.0;
    let mut ev_end = 0.0;
    let mut miss_rate = 0.0;
    let mut shard_rate = 0.0;
    let mut card_rate = 0.0;
    for (i, x) in rates.iter().enumerate() {
        let p = co.comp[i];
        if x.kind == "miss" {
            miss_rate += p;
            continue;
        }
        if x.kind == "shard" {
            shard_rate += p;
            let add = p * x.shards * sv;
            ev_new += add;
            ev_end += add;
            continue;
        }
        card_rate += p;
        ev_new += p * x.value;
        ev_end += p * x.dupe * sv;
    }
    let cost = f64::max(1.0, cost);
    EconomicsResult {
        base: co.base,
        gold_base: co.gold_base,
        comp: co.comp,
        gold_comp: co.gold_comp,
        cycle: co.cycle,
        hard_prob: co.hard_prob,
        up_rate: co.up_rate,
        up_share: co.up_share,
        ev_comp: co.ev_comp,
        degenerate: co.degenerate,
        sv,
        sv_from,
        ev_new,
        ev_end,
        miss_rate,
        shard_rate,
        card_rate,
        cost,
        ratio_new: ev_new / cost,
        ratio_end: ev_end / cost,
        ratio_worst: f64::max(ev_new, ev_end) / cost,
    }
}
