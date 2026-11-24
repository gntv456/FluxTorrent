/* 钓鱼玩法 · 配置数据层
 * 纯数据，无副作用、不依赖 DOM；浏览器用 window.FTData，Node 用 globalThis.FTData。
 * 所有价格/概率/上限集中在此，方便策划调表；引擎只读这里，不写死数字。
 */
(function (G) {
  'use strict';

  /* ---------- 稀有度 ---------- */
  // weight: 基础抽中权重；mult: 魔力价值系数；strength: 溜鱼拉力强度系数
  const RARITY = {
    common: { key: 'common', name: '普通', color: '#8a9ab0', weight: 520, mult: 1.0, strength: 1.0 },
    fine:   { key: 'fine',   name: '精良', color: '#4a7ab0', weight: 250, mult: 1.9, strength: 1.35 },
    rare:   { key: 'rare',   name: '稀有', color: '#6a7ab8', weight: 120, mult: 3.6, strength: 1.9 },
    epic:   { key: 'epic',   name: '史诗', color: '#b08a4a', weight: 45,  mult: 7.2, strength: 2.8 },
    legend: { key: 'legend', name: '传说', color: '#b86868', weight: 12,  mult: 15,  strength: 4.1 },
  };
  const RARITY_ORDER = ['common', 'fine', 'rare', 'epic', 'legend'];

  /* ---------- 打窝饵（消耗品，每竿 1 枚，2026-11-24 思齐对标） ----------
   * 与装备位的 bait（属性件，买断制）是两回事：打窝饵是抛竿时挂上的
   * 一次性消耗，按档位拉开该竿的稀有度权重。蚯蚓免费（自然恢复 + 签到
   * 赠送），高档饵用魔力购买 —— 这是钓场的主力魔力回收口。
   * rareBoost/epicBoost/legendBoost：对应稀有度权重乘数加成（小数）。
   */
  const BAITS = [
    { id: 'worm', name: '蚯蚓打窝饵', price: 0, desc: '免费基础饵，自然恢复', recover: true,
      eff: {} },
    { id: 'disguise', name: '伪装打窝饵', price: 1000, desc: '稀有鱼概率小幅提升', eff: { rare: 0.10 } },
    { id: 'bloodworm', name: '红虫打窝饵', price: 2000, desc: '更易遇见史诗鱼', eff: { rare: 0.12, epic: 0.15 } },
    { id: 'bladder', name: '泡囊打窝饵', price: 3000, desc: '吸引传说鱼', eff: { rare: 0.10, epic: 0.12, legend: 0.30 } },
    { id: 'insect', name: '昆虫打窝饵', price: 4000, desc: '高级鱼机会提升', eff: { rare: 0.15, epic: 0.18, legend: 0.20 } },
    { id: 'goldfish', name: '黄鱼打窝饵', price: 5000, desc: '向传说鱼招手', eff: { rare: 0.20, epic: 0.25, legend: 0.45 } },
  ];

  /* 放生回报（渔运）：按稀有度计点，渔运只可兑换解锁，永不可换魔力 */
  const RELEASE_LUCK = { common: 1, fine: 3, rare: 5, epic: 20, legend: 100 };

  /* ---------- 场景（含时段 / 天气三态） ---------- */
  // palettes: 各时段水体渐变；unlockAt 为 0 表示默认拥有
  const SCENES = [
    {
      id: 'bay', name: '热带海湾', rarity: 'fine', price: 0,
      desc: '新手友好，常见鱼成群出没',
      palettes: {
        day:   ['#7ec8f0', '#4aa8d8', '#2a88c0'],
        night: ['#2f4b86', '#243a6b', '#16264a'],
        rain:  ['#8ea6b8', '#6d889c', '#4f6a80'],
      },
      conds: ['day', 'night', 'rain'],
    },
    {
      id: 'gorge', name: '冰雪峡谷', rarity: 'rare', price: 600,
      desc: '冷水鱼群涌升，需 Lv.2 鱼竿',
      palettes: {
        day:   ['#cfe6f5', '#a8cbe2', '#7fa8c6'],
        night: ['#3a4f7a', '#2b3d63', '#1b2848'],
        rain:  ['#b6c6d4', '#93a6b8', '#6f8598'],
      },
      conds: ['day', 'night', 'rain'],
    },
    {
      id: 'abyss', name: '月夜深潭', rarity: 'epic', price: 1500,
      desc: '月光鱼 / 深水鱼出没 · 夜钓加成',
      palettes: {
        day:   ['#5a7fa8', '#44658c', '#2f4c70'],
        night: ['#3a3470', '#2a2658', '#181640'],
        rain:  ['#55688a', '#41526f', '#2c3a54'],
      },
      conds: ['day', 'night', 'rain'],
    },
    {
      id: 'reef', name: '珊瑚秘境', rarity: 'legend', price: 3000,
      desc: '珊瑚鱼 ×2 · 传说鱼概率提升',
      palettes: {
        day:   ['#7fd8d0', '#4fb8b8', '#2f9098'],
        night: ['#2f5a70', '#22465a', '#15313f'],
        rain:  ['#7aa8a4', '#5c8884', '#446765'],
      },
      conds: ['day', 'night', 'rain'],
    },
  ];

  /* ---------- 装备分类 ---------- */
  const CATS = [
    { id: 'rod',   name: '鱼竿', icon: 'rod',   attrLabel: '起竿窗口 / 稀有鱼概率' },
    { id: 'bait',  name: '鱼饵', icon: 'bait',  attrLabel: '咬钩速度 / 稀有吸引' },
    { id: 'hook',  name: '鱼钩', icon: 'hook',  attrLabel: '中钩率 / 脱钩率' },
    { id: 'line',  name: '鱼线', icon: 'line',  attrLabel: '拉力 / 鱼群警觉 / 断线率' },
    { id: 'float', name: '鱼漂', icon: 'float', attrLabel: '灵敏度 / 咬钩信号' },
  ];

  /* ---------- 装备表 ----------
   * price: 首次购买魔力价；0 = 默认拥有
   * eff: 生效字段（见 engine.statsOf 汇总）
   *  - window    起竿窗口加成（ms）
   *  - biteSpeed 咬钩速度加成（小数）
   *  - hookRate  中钩率加成（小数）
   *  - escape    脱钩率降低（小数）
   *  - rare      稀有以上概率加成（小数）
   *  - legend    传说鱼概率加成（小数）
   *  - limit     断线阈值加成（点）
   *  - alert     鱼群警觉降低（小数，延长窗口）
   *  - pull      收线效率加成（小数）
   *  - bigFish   大鱼拉力承受提升（小数）
   *  - night     夜钓加成（小数）
   */
  const ITEMS = {
    rod: [
      { id: 'bamboo', name: '竹节手竿', rarity: 'common', price: 0, desc: '起竿窗口标准 · 新手入门', eff: {} },
      { id: 'stardust', name: '星尘碳素竿', rarity: 'fine', price: 300, desc: '起竿窗口 +200ms · 不易弯竿', eff: { window: 200, pull: 0.06 } },
      { id: 'moon', name: '月光传说竿', rarity: 'epic', price: 1200, desc: '起竿窗口 +400ms · 稀有鱼概率 +5%', eff: { window: 400, rare: 0.05, pull: 0.12 } },
      { id: 'dragon', name: '沧海龙鳞竿', rarity: 'legend', price: 2800, desc: '窗口 +600ms · 大鱼拉力 -15% · 传说鱼 +3%', eff: { window: 600, rare: 0.08, legend: 0.03, pull: 0.18, bigFish: 0.15 } },
    ],
    bait: [
      { id: 'bread', name: '面包屑', rarity: 'common', price: 0, desc: '吸引常见鱼 · 常备饵料', eff: {} },
      { id: 'worm', name: '蚯蚓饵', rarity: 'fine', price: 120, desc: '咬钩速度 +15%', eff: { biteSpeed: 0.15 } },
      { id: 'shrimp', name: '荧光虾饵', rarity: 'rare', price: 380, desc: '夜间 / 深水咬钩 +25%', eff: { biteSpeed: 0.12, night: 0.25, rare: 0.03 } },
      { id: 'honey', name: '魔法花蜜饵', rarity: 'epic', price: 900, desc: '全水域稀有鱼概率 +8%', eff: { rare: 0.08, biteSpeed: 0.1 } },
    ],
    hook: [
      { id: 'iron', name: '铁制直钩', rarity: 'common', price: 0, desc: '中钩率标准 · 易脱钩', eff: {} },
      { id: 'steel', name: '精钢倒刺钩', rarity: 'fine', price: 150, desc: '中钩率 +6% · 脱钩率 -4%', eff: { hookRate: 0.06, escape: 0.04 } },
      { id: 'phantom', name: '幻影细钩', rarity: 'rare', price: 420, desc: '中钩率 +10% · 脱钩率 -8%', eff: { hookRate: 0.10, escape: 0.08 } },
      { id: 'fang', name: '龙牙巨钩', rarity: 'legend', price: 1000, desc: '大中钩 +14% · 传说级解锁', eff: { hookRate: 0.14, escape: 0.10, legend: 0.02, bigFish: 0.06 } },
    ],
    line: [
      { id: 'cotton', name: '棉麻线', rarity: 'common', price: 0, desc: '拉力标准 · 鱼群易警觉', eff: {} },
      { id: 'nylon', name: '尼龙主线', rarity: 'fine', price: 100, desc: '拉力 +12%', eff: { limit: 6 } },
      { id: 'silk', name: '蛛丝隐身线', rarity: 'rare', price: 360, desc: '拉力 +20% · 警觉 -10%', eff: { limit: 10, alert: 0.10 } },
      { id: 'gold', name: '流光金线', rarity: 'epic', price: 880, desc: '拉力 +32% · 警觉 -15%', eff: { limit: 16, alert: 0.15, pull: 0.08 } },
    ],
    float: [
      { id: 'reed', name: '芦苇浮漂', rarity: 'common', price: 0, desc: '灵敏度标准', eff: {} },
      { id: 'balsa', name: '巴尔杉立式漂', rarity: 'fine', price: 130, desc: '灵敏度 +8% · 窗口 +80ms', eff: { window: 80 } },
      { id: 'glow', name: '夜光感应漂', rarity: 'rare', price: 400, desc: '夜钓窗口 +150ms · 解锁夜间鱼种', eff: { window: 120, night: 0.15 } },
      { id: 'crystal', name: '水晶星轨漂', rarity: 'epic', price: 950, desc: '窗口 +200ms · 稀有鱼概率 +2%', eff: { window: 200, rare: 0.02 } },
    ],
  };

  /* ---------- 套装（六件同套，含场景） ---------- */
  const SETS = [
    {
      id: 'bayNewbie', name: '海湾新手套', rarity: 'fine',
      scene: 'bay', parts: { rod: 'bamboo', bait: 'bread', hook: 'iron', line: 'cotton', float: 'reed' },
      bonuses: [{ k: 'biteSpeed', v: 0.05, label: '咬钩速度 +5%' }, { k: 'common', v: 0.05, label: '常见鱼概率 +5%' }],
    },
    {
      id: 'stardust', name: '星尘精良套', rarity: 'fine',
      scene: 'gorge', parts: { rod: 'stardust', bait: 'worm', hook: 'steel', line: 'nylon', float: 'balsa' },
      bonuses: [{ k: 'window', v: 150, label: '起竿窗口 +150ms' }, { k: 'hookRate', v: 0.08, label: '中钩率 +8%' }, { k: 'escape', v: 0.06, label: '脱钩率 -6%' }],
    },
    {
      id: 'moonEpic', name: '月夜史诗套', rarity: 'epic',
      scene: 'abyss', parts: { rod: 'moon', bait: 'shrimp', hook: 'phantom', line: 'silk', float: 'glow' },
      bonuses: [{ k: 'nightRare', v: 0.10, label: '夜钓稀有鱼 +10%' }, { k: 'deepFish', v: 0.12, label: '深水鱼概率 +12%' }],
    },
    {
      id: 'dragonLegend', name: '沧海传说套', rarity: 'legend',
      scene: 'reef', parts: { rod: 'dragon', bait: 'honey', hook: 'fang', line: 'gold', float: 'crystal' },
      bonuses: [{ k: 'legend', v: 0.05, label: '传说鱼 +5%' }, { k: 'bigFish', v: 0.15, label: '大鱼拉力 -15%' }, { k: 'all', v: 0.05, label: '全属性 +5%' }],
      needPrev: 'moonEpic',
    },
  ];

  /* ---------- 鱼种 ----------
   * scene + cond 决定出现池；value 为基准魔力；size 为体长区间(cm)；river: 拉力强度系数
   * nightOnly / bait: 特殊出没条件（用于图鉴提示）
   */
  const FISH = [
    // 热带海湾 · 白天
    { id: 'band', name: '蓝带鱼', scene: 'bay', cond: 'day', rarity: 'common', value: 16, size: [10, 18], river: 1.0 },
    { id: 'puffer', name: '河豚', scene: 'bay', cond: 'day', rarity: 'common', value: 14, size: [8, 16], river: 1.1 },
    { id: 'shell', name: '海螺', scene: 'bay', cond: 'day', rarity: 'common', value: 12, size: [4, 9], river: 0.7 },
    { id: 'bream', name: '鲷鱼', scene: 'bay', cond: 'day', rarity: 'fine', value: 38, size: [14, 24], river: 1.2 },
    { id: 'lemon', name: '柠檬灯鱼', scene: 'bay', cond: 'day', rarity: 'fine', value: 42, size: [7, 12], river: 1.1 },
    { id: 'coralfin', name: '珊瑚雀', scene: 'bay', cond: 'day', rarity: 'rare', value: 105, size: [18, 30], river: 1.6 },
    // 热带海湾 · 夜钓
    { id: 'krill', name: '磷虾', scene: 'bay', cond: 'night', rarity: 'common', value: 13, size: [3, 6], river: 0.6 },
    { id: 'sandy', name: '沙蟹', scene: 'bay', cond: 'night', rarity: 'fine', value: 44, size: [9, 15], river: 1.3 },
    { id: 'nightglow', name: '夜光鲳', scene: 'bay', cond: 'night', rarity: 'rare', value: 118, size: [16, 26], river: 1.7, needFloat: 'glow' },
    // 热带海湾 · 阴雨
    { id: 'frog', name: '青蛙', scene: 'bay', cond: 'rain', rarity: 'common', value: 15, size: [5, 10], river: 0.9 },
    { id: 'mudfish', name: '泥鳅', scene: 'bay', cond: 'rain', rarity: 'common', value: 12, size: [6, 12], river: 0.8 },
    { id: 'rainperch', name: '雨鳞鲫', scene: 'bay', cond: 'rain', rarity: 'fine', value: 36, size: [12, 20], river: 1.2 },

    // 冰雪峡谷 · 白天
    { id: 'icecod', name: '冰纹鳕', scene: 'gorge', cond: 'day', rarity: 'fine', value: 46, size: [22, 38], river: 1.5 },
    { id: 'snowtrout', name: '雪鲑', scene: 'gorge', cond: 'day', rarity: 'fine', value: 52, size: [26, 42], river: 1.6 },
    { id: 'silvers', name: '银鳞鲟', scene: 'gorge', cond: 'day', rarity: 'rare', value: 132, size: [40, 68], river: 2.1 },
    { id: 'coldcarp', name: '寒潭鲤', scene: 'gorge', cond: 'day', rarity: 'common', value: 20, size: [18, 30], river: 1.3 },
    // 冰雪峡谷 · 夜钓
    { id: 'frostbass', name: '霜月鲈', scene: 'gorge', cond: 'night', rarity: 'rare', value: 140, size: [24, 40], river: 2.0 },
    { id: 'aurora', name: '极光鲷', scene: 'gorge', cond: 'night', rarity: 'epic', value: 330, size: [30, 52], river: 2.7 },
    { id: 'iceeel', name: '冰晶鳗', scene: 'gorge', cond: 'night', rarity: 'fine', value: 56, size: [50, 90], river: 1.9 },
    // 冰雪峡谷 · 阴雨
    { id: 'snowmelt', name: '雪水鳟', scene: 'gorge', cond: 'rain', rarity: 'fine', value: 48, size: [20, 34], river: 1.4 },
    { id: 'frostcat', name: '冻湖鲶', scene: 'gorge', cond: 'rain', rarity: 'common', value: 22, size: [28, 46], river: 1.5 },
    { id: 'whiteflake', name: '白霜鲽', scene: 'gorge', cond: 'rain', rarity: 'rare', value: 126, size: [16, 28], river: 1.8 },

    // 月夜深潭 · 白天
    { id: 'deepcat', name: '深潭鲶', scene: 'abyss', cond: 'day', rarity: 'fine', value: 58, size: [40, 70], river: 2.0 },
    { id: 'stonebass', name: '石斑鲈', scene: 'abyss', cond: 'day', rarity: 'rare', value: 148, size: [30, 52], river: 2.2 },
    { id: 'shadefish', name: '幽影鳅', scene: 'abyss', cond: 'day', rarity: 'common', value: 24, size: [10, 18], river: 1.1 },
    // 月夜深潭 · 夜钓
    { id: 'moonfish', name: '月光鱼', scene: 'abyss', cond: 'night', rarity: 'epic', value: 372, size: [24, 40], river: 2.8 },
    { id: 'crystaljelly', name: '晶母水母', scene: 'abyss', cond: 'night', rarity: 'epic', value: 340, size: [18, 32], river: 2.4 },
    { id: 'lantern', name: '深渊灯笼鱼', scene: 'abyss', cond: 'night', rarity: 'rare', value: 156, size: [14, 24], river: 2.0 },
    { id: 'nightowl', name: '夜枭鲷', scene: 'abyss', cond: 'night', rarity: 'fine', value: 62, size: [26, 44], river: 1.9 },
    // 月夜深潭 · 阴雨
    { id: 'bottomEel', name: '潭底鳗', scene: 'abyss', cond: 'rain', rarity: 'fine', value: 54, size: [60, 110], river: 2.1 },
    { id: 'darkflow', name: '暗流鲽', scene: 'abyss', cond: 'rain', rarity: 'rare', value: 138, size: [22, 38], river: 2.0 },
    { id: 'sunkcat', name: '沉木鲶', scene: 'abyss', cond: 'rain', rarity: 'common', value: 26, size: [34, 58], river: 1.6 },

    // 珊瑚秘境 · 白天
    { id: 'purplectopus', name: '紫章鱼', scene: 'reef', cond: 'day', rarity: 'rare', value: 168, size: [26, 44], river: 2.3 },
    { id: 'youngdragon', name: '幼龙鱼', scene: 'reef', cond: 'day', rarity: 'legend', value: 760, size: [46, 78], river: 3.6 },
    // 珊瑚秘境 · 夜钓
    { id: 'coralDragon', name: '珊瑚龙', scene: 'reef', cond: 'night', rarity: 'legend', value: 920, size: [56, 96], river: 4.0 },
    { id: 'starbass', name: '星轨鲷', scene: 'reef', cond: 'night', rarity: 'epic', value: 398, size: [30, 50], river: 2.9 },
    // 珊瑚秘境 · 阴雨
    { id: 'raincrab', name: '熔珊瑚蟹', scene: 'reef', cond: 'rain', rarity: 'fine', value: 66, size: [12, 20], river: 1.7 },
    { id: 'shellsole', name: '彩贝鲽', scene: 'reef', cond: 'rain', rarity: 'rare', value: 172, size: [20, 34], river: 2.2 },
  ];

  /* ---------- 体力 / 签到 / 背包 / 社交等常量 ---------- */
  const CONST = {
    staminaMax: 12,
    staminaRecoverMs: 6 * 60 * 1000,   // 每 6 分钟 +1
    staminaRefillCost: 40,             // 魔力补满
    castStaminaCost: 1,
    baitMaxStack: 999,          // 单种打窝饵库存上限
    bagTiers: [
      { cap: 30, price: 0, label: '默认容量', skin: '' },
      { cap: 50, price: 800, label: '扩容至 50 格', skin: '' },
      { cap: 80, price: 2000, label: '扩容至 80 格', skin: '' },
      { cap: 120, price: 5000, label: '满阶 · 120 格', skin: '专属背包皮肤' },
    ],
    aquariumCapPerTier: 8,              // 临时鱼缸容量 = 档位 * 8
    autoUnlockPrice: 1500,              // 自动垂钓买断价
    orderAllBonus: 300,                 // 当日全部订单完成的额外魔力
    giftDailyLimit: 5,                  // 每日赠饵次数
    giftReceiveCap: 6,                  // 待收鱼饵上限
    seasonDays: 14,                     // 赛季天数
    signInRewards: [
      { day: 1, type: 'magic', value: 20, label: '✦20' },
      { day: 2, type: 'stamina', value: 2, label: '体力+2' },
      { day: 3, type: 'magic', value: 80, label: '✦80' },
      { day: 4, type: 'bait', value: 6, label: '鱼饵×6' },
      { day: 5, type: 'stamina', value: 3, label: '体力+3' },
      { day: 6, type: 'magic', value: 150, label: '✦150' },
      { day: 7, type: 'chest', value: 400, label: '传说饵箱' },
    ],
    upgradeMaxLv: 5,
    // 养成单级花费 = 装备原价 * 该系数 * 当前等级；满级收益：窗口 +30ms/级、拉力 +2/级等
    upgradeCostRate: 0.45,
  };

  /* ---------- 成就 ---------- */
  // metric 取自 stats / album 派生；target 达成值；reward 魔力
  const ACH = [
    { id: 'firstCatch', name: '初钓者', group: 'fishing', icon: 'fish', desc: '钓上第一条鱼', metric: 'catches', target: 1, reward: 20 },
    { id: 'catch50', name: '渐入佳境', group: 'fishing', icon: 'fish', desc: '累计垂钓 50 尾', metric: 'catches', target: 50, reward: 60 },
    { id: 'catch100', name: '垂钓百尾', group: 'fishing', icon: 'fish', desc: '累计垂钓 100 尾', metric: 'catches', target: 100, reward: 100 },
    { id: 'catch500', name: '老练渔夫', group: 'fishing', icon: 'fish', desc: '累计垂钓 500 尾', metric: 'catches', target: 500, reward: 260 },
    { id: 'cast100', name: '勤抛竿', group: 'fishing', icon: 'rod', desc: '抛竿 100 次', metric: 'casts', target: 100, reward: 80 },
    { id: 'rare1', name: '稀有猎手', group: 'fishing', icon: 'star', desc: '钓获 1 尾稀有鱼', metric: 'rareCaught', target: 1, reward: 40 },
    { id: 'rare10', name: '稀有收藏家', group: 'fishing', icon: 'star', desc: '累计 10 尾稀有以上', metric: 'rareCaught', target: 10, reward: 150 },
    { id: 'epic1', name: '史诗垂钓', group: 'fishing', icon: 'star', desc: '钓获 1 尾史诗鱼', metric: 'epicCaught', target: 1, reward: 90 },
    { id: 'legend1', name: '传说降临', group: 'fishing', icon: 'medal', desc: '钓获 1 尾传说鱼', metric: 'legendCaught', target: 1, reward: 300 },
    { id: 'night10', name: '深夜行者', group: 'fishing', icon: 'moon', desc: '夜钓 10 尾', metric: 'nightCatches', target: 10, reward: 120 },
    { id: 'rain10', name: '雨中垂钓', group: 'fishing', icon: 'rain', desc: '阴雨 10 尾', metric: 'rainCatches', target: 10, reward: 120 },
    { id: 'bigfish', name: '大鱼克星', group: 'fishing', icon: 'bolt', desc: '钓获 1 尾 60cm 以上', metric: 'bigCaught', target: 1, reward: 100 },
    { id: 'nobreak', name: '稳如磐石', group: 'fishing', icon: 'check', desc: '连续 10 次不断线', metric: 'bestStreak', target: 10, reward: 140 },
    { id: 'album6', name: '图鉴入门', group: 'album', icon: 'album', desc: '图鉴收录 6 种', metric: 'albumCount', target: 6, reward: 50 },
    { id: 'album12', name: '图鉴爱好者', group: 'album', icon: 'album', desc: '图鉴收录 12 种', metric: 'albumCount', target: 12, reward: 100 },
    { id: 'album18', name: '图鉴达人', group: 'album', icon: 'album', desc: '图鉴收录 18 种', metric: 'albumCount', target: 18, reward: 150 },
    { id: 'album30', name: '博物学者', group: 'album', icon: 'album', desc: '图鉴收录 30 种', metric: 'albumCount', target: 30, reward: 320 },
    { id: 'scene2', name: '走出海湾', group: 'explore', icon: 'scene', desc: '解锁 2 个钓场', metric: 'sceneCount', target: 2, reward: 60 },
    { id: 'scene4', name: '四海钓客', group: 'explore', icon: 'scene', desc: '解锁全部 4 个钓场', metric: 'sceneCount', target: 4, reward: 260 },
    { id: 'set1', name: '第一套装备', group: 'grow', icon: 'set', desc: '首次激活渔具套装', metric: 'setActive', target: 1, reward: 120 },
    { id: 'gear5', name: '装备收藏家', group: 'grow', icon: 'rod', desc: '拥有 10 件渔具', metric: 'gearOwned', target: 10, reward: 90 },
    { id: 'lv5', name: '匠心打磨', group: 'grow', icon: 'star', desc: '任意装备养成至 Lv.5', metric: 'maxLv', target: 5, reward: 150 },
    { id: 'bag80', name: '行囊渐丰', group: 'grow', icon: 'bag', desc: '背包扩容至 80 格', metric: 'bagTier', target: 2, reward: 120 },
    { id: 'bag120', name: '满载而归', group: 'grow', icon: 'bag', desc: '背包扩容至 120 格', metric: 'bagTier', target: 3, reward: 300 },
    { id: 'order5', name: '订单常客', group: 'daily', icon: 'clock', desc: '完成 5 笔每日订单', metric: 'ordersDone', target: 5, reward: 80 },
    { id: 'order20', name: '委托专家', group: 'daily', icon: 'clock', desc: '完成 20 笔每日订单', metric: 'ordersDone', target: 20, reward: 200 },
    { id: 'sign7', name: '签到全勤', group: 'daily', icon: 'gift', desc: '完成一轮 7 日签到', metric: 'signFull', target: 1, reward: 150 },
    { id: 'gift5', name: '社交达人', group: 'social', icon: 'friend', desc: '累计赠送鱼饵 5 次', metric: 'giftsSent', target: 5, reward: 80 },
    { id: 'magic20k', name: '富甲一方', group: 'wealth', icon: 'sparkle', desc: '累计获得 20,000 魔力', metric: 'magicEarned', target: 20000, reward: 300 },
    { id: 'play100', name: '百折不挠', group: 'wealth', icon: 'sparkle', desc: '累计游戏 100 局', metric: 'casts', target: 100, reward: 60 },
  ];

  /* ---------- 好友（离线模拟，确定性生成） ---------- */
  const FRIENDS = [
    { id: 'f1', name: '潮汐猎人', tag: '潮', tone: 'sky', scene: 'abyss', rod: '月光传说竿', seed: 11 },
    { id: 'f2', name: '月下钓客', tag: '月', tone: 'lilac', scene: 'reef', rod: '星尘碳素竿', seed: 23 },
    { id: 'f3', name: '深潭老叟', tag: '潭', tone: 'sun', scene: 'abyss', rod: '沧海龙鳞竿', seed: 37 },
    { id: 'f4', name: '晚风垂钓', tag: '晚', tone: 'mint', scene: 'bay', rod: '竹节手竿', seed: 41 },
    { id: 'f5', name: '河畔萤火', tag: '萤', tone: 'coral', scene: 'gorge', rod: '荧光虾饵', seed: 53 },
    { id: 'f6', name: '礁石听涛', tag: '礁', tone: 'deep', scene: 'reef', rod: '沧海龙鳞竿', seed: 67 },
  ];

  /* ---------- 排行对手（赛季模拟） ---------- */
  const RIVALS = [
    { id: 'r1', name: '潮汐猎人', tag: '潮', tone: 'sun', base: 190, growth: 9 },
    { id: 'r2', name: '月下钓客', tag: '月', tone: 'sky', base: 176, growth: 8 },
    { id: 'r3', name: '深潭老叟', tag: '潭', tone: 'lilac', base: 162, growth: 7.4 },
    { id: 'r4', name: '晚风垂钓', tag: '晚', tone: 'mint', base: 142, growth: 6.2 },
    { id: 'r5', name: '河畔萤火', tag: '萤', tone: 'coral', base: 128, growth: 5.6 },
    { id: 'r6', name: '礁石听涛', tag: '礁', tone: 'deep', base: 112, growth: 5.0 },
    { id: 'r7', name: '雾隐舟', tag: '雾', tone: 'sky', base: 96, growth: 4.4 },
    { id: 'r8', name: '溪畔独钓', tag: '溪', tone: 'mint', base: 82, growth: 3.8 },
  ];

  const RANK_REWARDS = [500, 300, 200, 120, 120, 80, 80, 50];

  G.FTData = {
    RARITY, RARITY_ORDER, BAITS, RELEASE_LUCK, SCENES, CATS, ITEMS, SETS, FISH, CONST, ACH,
    FRIENDS, RIVALS, RANK_REWARDS,
    // 便捷索引
    fishById: FISH.reduce((m, f) => (m[f.id] = f, m), {}),
    itemById: Object.keys(ITEMS).reduce((m, cat) => {
      ITEMS[cat].forEach((it) => { m[it.id] = { ...it, cat }; });
      return m;
    }, {}),
    sceneById: SCENES.reduce((m, s) => (m[s.id] = s, m), {}),
    catById: CATS.reduce((m, c) => (m[c.id] = c, m), {}),
    setById: SETS.reduce((m, s) => (m[s.id] = s, m), {}),
  };
})(typeof window !== 'undefined' ? window : globalThis);
