//! 农场土地阶梯：买地 + 地块升级。
//!
//! **经济定位**：这一刀是**确定性魔力沉没口** —— 只出不进、零退款，不参与任何
//! EV 判据。升级买到的只有「周转」：产量、种子价、彩蛋倍率都与等级无关，所以
//! 收获期望恒等于作物表标定的 `super::farm::BASE_EV`（0.90）。一旦有人把等级
//! 接到价值侧（多产、多抽彩蛋），阶梯就从沉没口变成增发口，且越有钱的玩家
//! 越白赚 —— 下面的 `upgrading_buys_turnaround_not_value` 就是钉住这条线的闸。
//!
//! 两个阶梯的**底数与比率都是站长参数**（site_settings，见迁移 0256 的
//! settings_meta 行）；这里的常量只是缺省值，与 `eco_i64` 的缺省同源。

/// 一级到满级的周转步长（千分比）：每升一级，成熟时长 ×0.94
pub const SPEED_STEP_PERMILLE: i64 = 940;
/// 周转地板：升到底也不许把成熟时长压到一半以下（防止地块变成事实上的永动机，
/// 也保证等级上限能由地板**推出来**而不是手写一个数）
pub const SPEED_FLOOR_PERMILLE: i64 = 500;
/// 未升级地块的周转系数
pub const SPEED_BASE_PERMILLE: i64 = 1000;

/// 地块缺省价阶梯：底数（第 7 块地起）与比率（每多买一块 ×2.2）
pub const DEFAULT_LAND_BASE: i64 = 2000;
pub const DEFAULT_LAND_RATIO: i64 = 2200;
/// 升级缺省价阶梯：第一次升级 = 底数，之后每级 ×1.6
pub const DEFAULT_UP_BASE: i64 = 1000;
pub const DEFAULT_UP_RATIO: i64 = 1600;
/// 站长可配的总地块数缺省上限（免费 6 块之外还能买 6 块）。
/// 是 i64：它直接当作 site_settings 的缺省喂给 `eco_i64`。
pub const DEFAULT_MAX_PLOTS: i64 = 12;
/// 列宽与 CHECK 的硬上限：站长把上限调到比这块更大的数没有意义，
/// 而且阶梯价会在这个量级溢出成笑话
pub const MAX_PLOTS_HARD_CAP: i32 = 30;
/// 阶梯比率下限：比率 ≤ 1000 意味着「越买越便宜」，那不是阶梯
pub const RATIO_MIN: i64 = 1001;

/// 第 `level` 级的**未截断**周转系数（1000‰ 起步，逐级 ×940‰）
const fn uncapped_speed(level: i32) -> i64 {
    let mut p = SPEED_BASE_PERMILLE;
    let mut l = 1;
    while l < level {
        p = p * SPEED_STEP_PERMILLE / SPEED_BASE_PERMILLE;
        l += 1;
    }
    p
}

/// 最后一级的定义就是「再升一级也踩不到地板之下」：地板一压住，下一级的
/// 系数与这一级完全相同，那一注钱什么也没买到。上限由地板推出，两者不会走偏。
const fn last_useful_level() -> i32 {
    let mut l = 1;
    loop {
        let next = uncapped_speed(l + 1);
        if next <= SPEED_FLOOR_PERMILLE || next >= uncapped_speed(l) {
            return l;
        }
        l += 1;
    }
}

/// 地块等级上限（由 `SPEED_FLOOR_PERMILLE` 推出，非手写常数）
pub const MAX_LEVEL: i32 = last_useful_level();

#[derive(Debug, Clone, PartialEq)]
pub enum LandError {
    /// 站长把阶梯底数配成 0 或负数
    BadBase(i64),
    /// 阶梯比率不增（≤1000‰）
    BadRatio(i64),
    /// 地块数已到达站长设的上限
    NoRoom { owned: i32, cap: i32 },
    /// 该槽位已经持有（或本来就是免费地）
    AlreadyOwned(i32),
    /// 槽位不在可买区间：只能按阶梯一块一块往外买
    BadSlot { slot: i32, next: i32 },
    /// 等级已是上限，再升什么都不买到
    MaxLevel { level: i32 },
    /// 站长把总地块上限配出了可用区间
    BadCap(i64),
    /// 上限低于免费地块数：配置自己说不通（人人都已经超出上限）
    CapBelowFree { cap: i64, free: i32 },
}

impl std::fmt::Display for LandError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            LandError::BadBase(b) => {
                write!(f, "阶梯底数需大于 0，实为 {b}（配成 0 等于白送地块）")
            }
            LandError::BadRatio(r) => write!(
                f,
                "阶梯比率需大于 {RATIO_MIN}‰（越买越贵），实为 {r}‰"
            ),
            LandError::NoRoom { owned, cap } => write!(
                f,
                "地块已买满（{owned}/{cap}），站长把上限就放在这里"
            ),
            LandError::AlreadyOwned(s) => {
                write!(f, "第 {s} 号地块你已经有了，不用重复买")
            }
            LandError::BadSlot { slot, next } => write!(
                f,
                "地块要按阶梯一块一块往外买：现在只能买第 {next} 号，\
                 你要买的是第 {slot} 号"
            ),
            LandError::MaxLevel { level } => write!(
                f,
                "地块等级已到上限 {level}：再升一级也踩在周转地板上，\
                 这一注钱什么都买不到"
            ),
            LandError::BadCap(c) => write!(
                f,
                "总地块上限需在 1 ~ {MAX_PLOTS_HARD_CAP} 之间，实为 {c}：\
                 上限是阶梯能走多远的地方，配成天文数字只会让价溢出"
            ),
            LandError::CapBelowFree { cap, free } => write!(
                f,
                "总地块上限 {cap} 低于免费送的地块数 {free}：\
                 每个人一进门就已经超出上限，买地这条阶梯就没有意义了。\
                 要收地请改代码里的免费数，别把上限调到免费数以下"
            ),
        }
    }
}

/// 阶梯价：`base × (ratio/1000)^steps`，逐级向下取整（取整侧对站点有利）。
/// `steps` 是「已经买过的档数」，0 表示第一档 = 底数本身。
///
/// 参数**必须由 `validate_ladder` 先过关**：这里不做静默纠正 —— 一个被
/// clamp 成正数的 0 底数，配面上看是 2000 而实际免费，正是最难查的那类错。
/// 末尾的 `max(1)` 不是纠正，是「地块不许白送」这条规则本身。
pub fn ladder_price(base: i64, ratio_permille: i64, steps: i32) -> i64 {
    let mut p = base;
    let mut n = 0;
    while n < steps {
        p = p.saturating_mul(ratio_permille) / SPEED_BASE_PERMILLE;
        n += 1;
    }
    p.max(1)
}

/// 站长参数关闸：底数与比率任一不合法都会让阶梯失去意义。
/// `cap` 收 **i64**（site_settings 读出来就是 i64）：先在这里判区间，
/// 调用方才许 `as i32` —— 越界值如果先转型再判，`3_000_000_000` 会绕成
/// 负数、`2_147_483_653` 会绕成 5，正好落进合法区间被放过去。
pub fn validate_ladder(
    base: i64,
    ratio_permille: i64,
    cap: i64,
) -> Result<(), LandError> {
    if base < 1 {
        return Err(LandError::BadBase(base));
    }
    if ratio_permille < RATIO_MIN {
        return Err(LandError::BadRatio(ratio_permille));
    }
    if cap < 1 || cap > i64::from(MAX_PLOTS_HARD_CAP) {
        return Err(LandError::BadCap(cap));
    }
    Ok(())
}

/// 一次购买/升级要用到的全部站长参数（已判过区间，i32 字段是安全的）
#[derive(Debug, Clone, PartialEq)]
pub struct LandConfig {
    /// 总地块数上限（含免费的 `super::farm::PLOTS` 块）
    pub max_plots: i32,
    pub land_base: i64,
    pub land_ratio: i64,
    pub up_base: i64,
    pub up_ratio: i64,
}

/// site_settings 的五把数落成可用配置的**唯一**入口：一起过关，
/// 并把上限与免费地块数对齐（上限低于免费数意味着配置本身说不通）。
pub fn land_config(
    max_plots: i64,
    land_base: i64,
    land_ratio: i64,
    up_base: i64,
    up_ratio: i64,
) -> Result<LandConfig, LandError> {
    validate_ladder(land_base, land_ratio, max_plots)?;
    validate_ladder(up_base, up_ratio, max_plots)?;
    let free = super::farm::PLOTS;
    if max_plots < i64::from(free) {
        return Err(LandError::CapBelowFree {
            cap: max_plots,
            free,
        });
    }
    Ok(LandConfig {
        max_plots: max_plots as i32,
        land_base,
        land_ratio,
        up_base,
        up_ratio,
    })
}

/// 第 `level` 级的周转系数（千分比），压在地板之上。
/// 超过上限的等级一律按地板算 —— 调用方应先 `validate_upgrade` 拒绝它，
/// 这里兜底只是为了让「越界等级」不会反过来把成熟时长抬高。
pub fn speed_permille(level: i32) -> i64 {
    uncapped_speed(level.max(1)).max(SPEED_FLOOR_PERMILLE)
}

/// 成熟分钟数：作物表的 `grow_hours` × 周转系数。**唯一**受等级影响的量。
/// 保留至少 1 分钟，防止低时长作物被等级压成 0 而变成「种下即熟」。
pub fn grow_minutes(grow_hours: i32, level: i32) -> i64 {
    let raw = grow_hours.max(0) as i64 * 60 * speed_permille(level)
        / SPEED_BASE_PERMILLE;
    raw.max(1)
}

/// 下一块可买的地块槽位：免费地是 `1..=free`，买到的地按阶梯连续排下去
///（`free+1`、`free+2`……）。连续是刻意的：一旦允许跳号买，「买满」就没有
/// 可判定的口径，阶梯价也无从对齐。
pub fn next_purchasable_slot(
    free: i32,
    purchased: i32,
    cap: i32,
) -> Result<i32, LandError> {
    if free + purchased >= cap {
        return Err(LandError::NoRoom {
            owned: free + purchased,
            cap,
        });
    }
    Ok(free + purchased + 1)
}

/// 买第 n 块地（`purchased` 为已购块数）的价格。
pub fn land_price(
    base: i64,
    ratio_permille: i64,
    purchased: i32,
) -> i64 {
    ladder_price(base, ratio_permille, purchased.max(0))
}

/// 把某块地从 `level` 升到 `level + 1` 的价格。
pub fn upgrade_price(
    base: i64,
    ratio_permille: i64,
    level: i32,
) -> i64 {
    ladder_price(base, ratio_permille, (level - 1).max(0))
}

/// 升级关闸：满级之后不收钱，因为那一注钱买不到任何东西。
/// 「买不到东西却收费」在别处叫坑钱，在这里叫破坏周转地板的定义。
pub fn validate_upgrade(level: i32) -> Result<(), LandError> {
    if level >= MAX_LEVEL {
        return Err(LandError::MaxLevel { level });
    }
    Ok(())
}

/// 持有关闸：槽位必须是自己已经买到的那一个（免费地不需要持有行）。
pub fn validate_purchase(
    slot: i32,
    free: i32,
    purchased: i32,
    cap: i32,
) -> Result<i32, LandError> {
    let next = next_purchasable_slot(free, purchased, cap)?;
    if slot < next {
        return Err(LandError::AlreadyOwned(slot));
    }
    if slot != next {
        return Err(LandError::BadSlot { slot, next });
    }
    Ok(next)
}
