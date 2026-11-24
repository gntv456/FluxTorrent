//! 打地鼠：纯计算层（排程 / 窗口判时机 / 结算），不引 sqlx。
//!
//! 与其余玩法的一局一抽不同，这一局是「多次冒头」：
//!   · 服务端在 start 时生成整局排程（时间、洞位、档位、价值）并落库；
//!   · 每次敲击只判**时机**，不再掷骰子（同钓鱼：结果在开局定死）；
//!   · 结算只认服务端记下的命中，客户端报的分数不作数。
//!
//! 经济闸不是「单次 EV < 1」，而是「满盘全中 < 票价 × 95%」：
//! 一局 12 次奖池冒头，若单次 EV 0.5，手速打满就能拿 6 倍票价 ——
//! 只看单次 EV 会把增发口放过去。`validate_mole` 判的就是这道满盘闸。

use rand::Rng;

use super::jgg::{ev_strictly_below, validate_pool, EntryKind, PoolEntry};

/// 一局时长（前端倒计时同源）
pub const ROUND_MS: i64 = 30_000;
/// 一局奖池冒头次数（炸弹另算）
pub const POOL_SPAWNS: usize = 12;
/// 一局炸弹个数：只扣不加，故不参与满盘闸
pub const BOMB_SPAWNS: usize = 2;
/// 冒头间隔：一只下去 0.4 秒，下一只才出来
pub const SPAWN_GAP_MS: i64 = 1_900;
/// 第一只的起手时间，留给「开始」动画
pub const FIRST_SPAWN_MS: i64 = 600;
/// 单只停留时长
pub const UP_MS: i64 = 1_500;
/// 炸弹默认扣分（可被站点设置覆盖，开局写进排程）
pub const DEFAULT_BOMB_PENALTY: i64 = 25;
/// 判时机容差：单程网络 + 人类反应。早到 600ms 也算中，
/// 晚到 900ms 也算中 —— 宁可放宽，不能把正常玩家判成挂。
pub const HIT_EARLY_MS: i64 = 600;
pub const HIT_LATE_MS: i64 = 900;
/// 满盘上限：Σ 档位价值 × 冒头数 < 票价 × 95%
pub const CEIL_NUM: i64 = 95;
pub const CEIL_DEN: i64 = 100;
/// 洞位数（前端 3×3 同源）
pub const HOLES: u8 = 9;

/// 一次冒头。`idx` 是奖池下标，炸弹为 -1（label 留空由前端本地化）。
/// 直接以 JSON 落库：start 写、hit/finish 读回，字段是同一份。
#[derive(Debug, Clone, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
pub struct Spawn {
    pub seq: i32,
    pub at: i64,
    pub up: i64,
    pub hole: u8,
    pub idx: i32,
    /// 奖池 meta 里的稀有度，图鉴/播报按它分色
    #[serde(default = "one")]
    pub rarity: i16,
    pub label: String,
    pub value: i64,
}

fn one() -> i16 {
    1
}

/// 满盘闸 + 结构校验。写侧（后台保存）与读侧（load_pool）共用一份，
/// 避免「面板说能存、玩法拒绝服务」的两套口径。
pub fn validate_mole(
    entries: &[PoolEntry],
    ticket: i64,
) -> Result<(), &'static str> {
    validate_pool(entries, ticket).map_err(|_| {
        "打地鼠奖池不合法：需非空、权重非零、且单次价值低于票价"
    })?;
    if entries
        .iter()
        .any(|e| matches!(e.kind, EntryKind::Item { .. }))
    {
        return Err("打地鼠只发魔力：奖池里不能配物品位");
    }
    // 12 次冒头 × 单次价值 < 票价 × 0.95
    if !ev_strictly_below(entries, ticket, CEIL_NUM, CEIL_DEN * 12) {
        return Err("打地鼠满盘全中会超过票价的 95%：请调低档位价值或调高票价");
    }
    Ok(())
}

/// 单只的窗口：`elapsed` 是服务端收到的毫秒数（自 start 起算）。
pub fn in_window(spawn: &Spawn, elapsed: i64) -> bool {
    elapsed >= spawn.at - HIT_EARLY_MS
        && elapsed <= spawn.at + spawn.up + HIT_LATE_MS
}

/// 命中合计 → 实付。合计可为负（炸弹多于好鼠），实付不低于 0。
pub fn settle_score(values: &[i64]) -> (i64, i64) {
    let score = values
        .iter()
        .copied()
        .fold(0i64, |a, b| a.saturating_add(b));
    (score, score.max(0))
}

/// 这一局的排程是否已经走完（最后一只的判中窗口已关闭）。
///
/// `finish` 只许在走完之后结算。此前不校验：玩家玩到一半切去「奖池」标签
/// （前端 `stop()` 会调 finish）或组件卸载，这一局就**当场按已命中的分数结算**
/// —— 后面还没冒头的地鼠（含大奖）全部作废，票价照扣。提前结算只会让玩家
/// 拿得更少，所以不是经济漏洞，但「切个标签页就白亏一局」足够伤人。
///
/// 排程解不出来（坏 JSON / 空排程）时返回 `false` —— 配合 [`round_over`]
/// 的时长闸一起用（见 [`settled_or_over`]）：坏排程的局不会因为「解析失败」
/// 就永远结不了，也不会在半途被结掉。
pub fn schedule_done(spawns: &[Spawn], elapsed_ms: i64) -> bool {
    let Some(last) = spawns.iter().map(|s| s.at + s.up + HIT_LATE_MS).max()
    else {
        return false;
    };
    elapsed_ms >= last
}

/// 整局时长闸：开局超过 `ROUND_MS` 的局，排程**必然**已走完。
///
/// 排程是开局时按固定节奏生成的（最后一只在
/// `FIRST_SPAWN_MS + 13 × SPAWN_GAP_MS + UP_MS + HIT_LATE_MS` ≈ 27.7s 收尾，
/// 早于 30s 整局），所以时长是不依赖排程内容的权威判据 —— 坏 JSON 的局
/// 也靠它收尾，而不是被「解析失败」卡成永远结不了的坏账。
pub fn round_over(elapsed_ms: i64) -> bool {
    elapsed_ms >= ROUND_MS
}

/// 这一局现在能不能结算：排程按内容判定走完，**或**整局时长已过。
/// 两边取「或」—— 正常局在最后一只收尾后（≈27.7s）即可结算，不必等满 30s；
/// 排程损坏的局在 30s 后由时长闸放行。
pub fn settled_or_over(spawns: &[Spawn], elapsed_ms: i64) -> bool {
    round_over(elapsed_ms) || schedule_done(spawns, elapsed_ms)
}

/// 生成整局排程。调用方必须先过 `validate_mole`。
///
/// 洞位刻意「不连续同洞」：同一洞连着冒两次，玩家会当成没打中。
pub fn build_schedule(
    entries: &[PoolEntry],
    ticket: i64,
    bomb_penalty: i64,
    rarities: &[i16],
    rng: &mut impl Rng,
) -> Vec<Spawn> {
    let total = POOL_SPAWNS + BOMB_SPAWNS;
    let mut bombs = [false; POOL_SPAWNS + BOMB_SPAWNS];
    let mut placed = 0;
    while placed < BOMB_SPAWNS {
        let i = rng.gen_range(0..total);
        if !bombs[i] {
            bombs[i] = true;
            placed += 1;
        }
    }
    let penalty = bomb_penalty.max(0);
    let mut prev_hole: Option<u8> = None;
    let mut out = Vec::with_capacity(total);
    for i in 0..total {
        let at = FIRST_SPAWN_MS + i as i64 * SPAWN_GAP_MS;
        let mut hole = rng.gen_range(0..HOLES);
        if Some(hole) == prev_hole {
            let step = rng.gen_range(1..HOLES);
            hole = (hole + step) % HOLES;
        }
        prev_hole = Some(hole);
        let spawn = if bombs[i] {
            Spawn {
                seq: i as i32,
                at,
                up: UP_MS,
                hole,
                idx: -1,
                rarity: 1,
                label: String::new(),
                value: -penalty,
            }
        } else {
            let idx = draw_index(entries, rng);
            let e = &entries[idx];
            Spawn {
                seq: i as i32,
                at,
                up: UP_MS,
                hole,
                idx: idx as i32,
                rarity: rarities.get(idx).copied().unwrap_or(1),
                label: e.label.clone(),
                value: e.value(ticket),
            }
        };
        out.push(spawn);
    }
    out
}

/// 按权重抽下标。`validate_mole` 已保证权重非空、合计不溢出。
fn draw_index(entries: &[PoolEntry], rng: &mut impl Rng) -> usize {
    let total: u64 = entries.iter().map(|e| u64::from(e.weight)).sum();
    let mut roll = rng.gen_range(0..total.max(1));
    for (i, e) in entries.iter().enumerate() {
        let w = u64::from(e.weight);
        if roll < w {
            return i;
        }
        roll -= w;
    }
    entries.len().saturating_sub(1)
}
