/**
 * 抽卡数学层 —— 样张 `.workbuddy/gacha-shared.js` 的 TS 镜像
 * （方案《抽卡玩法落地方案-2026-09-27》§3「必须先抽出来的一层」）。
 * 向量 `apps/api/src/gacha_math/gacha_math_vectors.json` 由样张生成，与
 * `apps/api/src/gacha_math/` 的 Rust 镜像共用（vitest 侧
 * `apps/web/tests/gacha-math.test.ts`；L7 互验先例）。改任何一行算术必须
 * 同步另一侧并用 `packages/gacha-math/scripts/gen-vectors.cjs` 重生成
 * （生成器对样张逐位比对，不一致即拒绝写向量）。与样张刻意出入：
 * svFrom 结构化（文案由展示层翻）；simulate 去掉 snaps/pulls 画图载荷。
 */

/** 「出金」= gold_rank ≥ 3（SSR 及以上）；真实实现以 gacha_rarities 行表为准（0226）。 */
export const GOLD_MIN_RANK = 3;

/** 样张的档位表；rank 只服务 isGold，颜色/星数等视觉 token 不进数学层。 */
const RANKS: Record<string, number> = { R: 1, SR: 2, SSR: 3, UR: 4, LR: 5 };

export function isGold(r: string): boolean {
  return (RANKS[r] ?? 0) >= GOLD_MIN_RANK;
}

export type OutputType = "card" | "shard" | "miss";

/** 奖池行。碎片/谢谢惠顾不是稀有度，`r` 用 SHARD/MISS 占位。 */
export interface RateRow {
  r: string;
  type: OutputType;
  /** 表定权重；综合概率由保底 DP 派生（composite），守卫与公示必须用后者。 */
  w: number;
  /** 卡的价值。 */
  value?: number;
  /** 碎片档：一次产出数量。 */
  shards?: number;
  /** 卡档：满图鉴后重复卡的折算价。 */
  dupe?: number;
  /** 卡档：合成所需碎片数 —— 碎片单价的倒推来源之一。 */
  synth?: number;
  lvMax?: number;
  lvStep?: number;
  lvGain?: number;
}

export interface PityConfig {
  soft: number;
  hard: number;
  /** 每抽抬升的百分点（把概率从非金档按比例搬到金档）。 */
  ramp: number;
  upRatio?: number;
  guaranteeUp?: boolean;
}

export interface PoolConfig {
  rates: RateRow[];
  pity: PityConfig;
}

export interface CompositeResult {
  base: number[];
  goldBase: number;
  /** 含保底的综合概率 —— 公示、守卫、EV 全用这份。 */
  comp: number[];
  goldComp: number;
  /** 一个保底周期的期望长度（抽数）。 */
  cycle: number;
  /** 走满硬保底的概率（纯装饰时的后台告警口径）。 */
  hardProb: number;
  upRate: number;
  upShare: number;
  evComp: number;
  /** 金档占比为 0 或 ≥1 时保底 DP 无意义，comp 退化为 base。 */
  degenerate: boolean;
}

/** 可复现 PRNG（mulberry32）：同种子必同结果，seed 落库以便事后复核。 */
export function rng(seed: number): () => number {
  let a = seed | 0;
  return () => {
    a = (a + 0x6d2b79f5) | 0;
    let t = Math.imul(a ^ (a >>> 15), a | 1);
    t = (t + Math.imul(t ^ (t >>> 7), 61 | t)) ^ t;
    return ((t ^ (t >>> 14)) >>> 0) / 4294967296;
  };
}

/** 保底 DP 精确解。状态 s = 周期内已抽数（1-based）：
 *  q(s) = s≥hard ? 1 : gs + boost(s)；h(s) = 期望经过 s 的次数；
 *  comp(i) = Σ_s h(s)·P(i|s) / Σ_s h(s)，质量守恒 Σ comp == 1。 */
export function composite(cfg: PoolConfig): CompositeResult {
  const rates = cfg.rates;
  const W = rates.reduce((a, x) => a + (+x.w || 0), 0) || 1;
  const p = rates.map((x) => (+x.w || 0) / W);
  const gs = p.reduce((a, x, i) => a + (isGold(rates[i]!.r) ? x : 0), 0);
  const soft = +cfg.pity.soft || 0;
  const hard = Math.max(1, +cfg.pity.hard || 1);
  const ramp = +cfg.pity.ramp || 0;
  const upRatio =
    cfg.pity.upRatio == null ? 1 : Math.min(1, Math.max(0, +cfg.pity.upRatio));
  const out: CompositeResult = {
    base: p, goldBase: gs, comp: p.slice(), goldComp: gs, cycle: 1,
    hardProb: 0, upRate: 0, upShare: 0, evComp: 0, degenerate: true,
  };
  if (gs <= 0 || gs >= 1 || !Number.isFinite(gs)) {
    out.evComp = rates.reduce(
      (a, x, i) => a + p[i]! * (+(x.value ?? NaN) || 0), 0);
    return out;
  }
  out.degenerate = false;

  const boost = (s: number) =>
    s > soft && s < hard ? Math.min(((s - soft) * ramp) / 100, 1 - gs) : 0;
  const q = (s: number) => (s >= hard ? 1 : Math.min(1, gs + boost(s)));
  const h: number[] = [];
  let cur = 1;
  for (let s = 1; s <= hard; s++) {
    h.push(cur);
    cur *= 1 - q(s);
  }
  const L = h.reduce((a, b) => a + b, 0) || 1;
  const nonGoldShare = 1 - gs;
  out.comp = p.map((pi, i) => {
    const gold = isGold(rates[i]!.r);
    return h.reduce((acc, hs, s) => {
      const st = s + 1;
      const b = boost(st);
      if (gold) return acc + hs * (st >= hard ? pi / gs : pi * (1 + b / gs));
      return acc + hs * (st >= hard ? 0
        : pi * (1 - (nonGoldShare ? b / nonGoldShare : 0)));
    }, 0) / L;
  });
  let hardProb = 1;
  for (let s = 1; s < hard; s++) hardProb *= 1 - q(s);
  const goldPulls = h.reduce((acc, hs, s) => acc + hs * q(s + 1), 0) / L;
  const upPulls = h.reduce((acc, hs, s) => {
    const st = s + 1;
    const r = st >= hard && cfg.pity.guaranteeUp ? 1 : upRatio;
    return acc + hs * q(st) * r;
  }, 0) / L;
  out.goldComp = out.comp.reduce(
    (a, x, i) => a + (isGold(rates[i]!.r) ? x : 0), 0);
  out.cycle = L;
  out.hardProb = hardProb;
  out.upShare = goldPulls ? upPulls / goldPulls : 0;
  out.upRate = upPulls;
  out.evComp = out.comp.reduce(
    (a, x, i) => a + x * (+(rates[i]?.value ?? NaN) || 0), 0);
  return out;
}

export interface SimulateResult {
  freq: number[];
  counts: number[];
  gaps: number[];
  maxGap: number;
  gold: number;
  hardHits: number;
  tenWin: number;
  tenTot: number;
  tenRate: number;
  evSim: number;
  evBase: number;
  goldBase: number;
  N: number;
}

/** 蒙特卡洛：与 composite 互为对照，两侧实现必须给出同一整数计数。 */
export function simulate(
  cfg: PoolConfig, N: number, seed: number,
): SimulateResult {
  const rates = cfg.rates;
  const base = rates.map((x) => x.w);
  const W = base.reduce((a, b) => a + b, 0) || 1;
  const p = base.map((x) => x / W);
  const gold = rates.map((x) => isGold(x.r));
  const gs = p.reduce((a, x, i) => a + (gold[i] ? x : 0), 0);
  const soft = +cfg.pity.soft || 0;
  const hard = Math.max(1, +cfg.pity.hard || 1);
  const ramp = +cfg.pity.ramp || 0;
  const rnd = rng(seed);
  const counts = new Array<number>(rates.length).fill(0);
  const gaps: number[] = [];
  let since = 0, goldN = 0, hardHits = 0, tenWin = 0, tenTot = 0, maxGap = 0;
  let batch: number[] = [];
  const nonGS = 1 - gs;
  for (let i = 1; i <= N; i++) {
    since++;
    let pick: number;
    if (since >= hard) {
      let r = rnd() * gs, acc = 0;
      pick = rates.length - 1;
      for (let k = 0; k < p.length; k++) {
        if (!gold[k]) continue;
        acc += p[k]!;
        if (r < acc) { pick = k; break; }
      }
      hardHits++;
    } else {
      const b = since > soft
        ? Math.min(((since - soft) * ramp) / 100, gs ? 1 - gs : 0) : 0;
      const probs = p.map((pi, k) => gold[k]
        ? pi * (1 + (gs ? b / gs : 0))
        : pi * (1 - (nonGS ? b / nonGS : 0)));
      const tot = probs.reduce((a, x) => a + Math.max(0, x), 0);
      let r = rnd() * tot, acc = 0;
      pick = probs.length - 1;
      for (let k = 0; k < probs.length; k++) {
        acc += Math.max(0, probs[k]!);
        if (r < acc) { pick = k; break; }
      }
    }
    counts[pick]! += 1;
    if (gold[pick]) {
      if (since > maxGap) maxGap = since;
      gaps.push(since);
      goldN++;
      since = 0;
    }
    batch.push(pick);
    if (batch.length === 10) {
      if (batch.some((x) => gold[x])) tenWin++;
      tenTot++;
      batch = [];
    }
  }
  const freq = counts.map((c) => c / N);
  return {
    freq, counts, gaps, maxGap, gold: goldN, hardHits,
    tenWin, tenTot, tenRate: tenTot ? tenWin / tenTot : 0,
    evSim: rates.reduce(
      (a, x, i) => a + freq[i]! * (+(x.value ?? NaN) || 0), 0),
    evBase: rates.reduce(
      (a, x, i) => a + p[i]! * (+(x.value ?? NaN) || 0), 0),
    goldBase: gs, N,
  };
}

export interface EconomicsResult extends CompositeResult {
  /** 碎片单价 = 逐卡取 max(value/synth, value·lvGain/升级总价)。 */
  sv: number;
  svFrom: { kind: "synth" | "level"; r: string } | null;
  evNew: number;
  evEnd: number;
  missRate: number;
  shardRate: number;
  cardRate: number;
  cost: number;
  ratioNew: number;
  ratioEnd: number;
  /** 守卫口径 = max(新手, 毕业)/cost；>1 即倒灌经济（合成价后门见方案 §1.3）。 */
  ratioWorst: number;
}

/** 奖池含碎片/谢谢惠顾与卡牌等级后的经济闭环。 */
export function economics(
  cfg: PoolConfig & { cost: number },
): EconomicsResult {
  const co = composite(cfg);
  const rates = cfg.rates;
  let sv = 0;
  let svFrom: { kind: "synth" | "level"; r: string } | null = null;
  rates.forEach((x) => {
    if (x.type !== "card") return;
    const v = +(x.value ?? NaN) || 0;
    const synth = +(x.synth ?? NaN) || 0;
    if (synth > 0) {
      const r = v / synth;
      if (r > sv) { sv = r; svFrom = { kind: "synth", r: x.r }; }
    }
    const lvStep = +(x.lvStep ?? NaN) || 0;
    const lvMax = +(x.lvMax ?? NaN) || 1;
    const lvCost = lvStep * Math.max(0, lvMax - 1);
    if (lvCost > 0 && +(x.lvGain ?? NaN) > 0) {
      const r = (v * +(x.lvGain ?? NaN)) / lvCost;
      if (r > sv) { sv = r; svFrom = { kind: "level", r: x.r }; }
    }
  });
  let evNew = 0, evEnd = 0, missRate = 0, shardRate = 0, cardRate = 0;
  rates.forEach((x, i) => {
    const p = co.comp[i]!;
    if (x.type === "miss") { missRate += p; return; }
    if (x.type === "shard") {
      shardRate += p;
      const add = p * (+(x.shards ?? NaN) || 0) * sv;
      evNew += add;
      evEnd += add;
      return;
    }
    cardRate += p;
    evNew += p * (+(x.value ?? NaN) || 0);
    evEnd += p * (+(x.dupe ?? NaN) || 0) * sv;
  });
  const cost = Math.max(1, +cfg.cost || 1);
  return {
    ...co, sv, svFrom, evNew, evEnd, missRate, shardRate, cardRate, cost,
    ratioNew: evNew / cost, ratioEnd: evEnd / cost,
    ratioWorst: Math.max(evNew, evEnd) / cost,
  };
}
