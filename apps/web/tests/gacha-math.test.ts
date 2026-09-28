import { describe, expect, it } from "vitest";
import {
  composite,
  economics,
  isGold,
  simulate,
  type CompositeResult,
  type EconomicsResult,
  type PoolConfig,
  type RateRow,
} from "../../../packages/gacha-math/src/index";
import vectors from "../../api/src/gacha_math/gacha_math_vectors.json";

/**
 * 抽卡数学（方案《抽卡玩法落地方案-2026-09-27》§3）的前端侧镜像测试。
 *
 * 这些用例是 `apps/api/src/gacha_math.rs` 那组单测的**逐条镜像**，两边共用同一份
 * 向量 `apps/api/src/gacha_math_vectors.json` —— 它由未入库样张 oracle 生成
 * （`packages/gacha-math/scripts/gen-vectors.cjs`，生成时已对样张逐位比对）。
 * TS 实现服务前端公示，Rust 实现服务守卫与运行时预检：任何一边算术漂了，
 * 同一个用例就会在另一侧红。改任何一侧都必须同步另一侧并重生成向量。
 */
interface VectorCase {
  name: string;
  rates: RateRow[];
  pity: PoolConfig["pity"];
  cost: number;
  expectedComposite: CompositeResult;
  expectedEconomics: EconomicsResult;
  simulate?: {
    n: number;
    seed: number;
    gold: number;
    hardHits: number;
    tenWin: number;
    tenTot: number;
    maxGap: number;
    counts: number[];
    evSim: number;
    evBase: number;
  };
}

const CASES = vectors.cases as unknown as VectorCase[];

const byName = (n: string): VectorCase => {
  const hit = CASES.filter((c) => c.name === n);
  if (hit.length !== 1 || !hit[0]) throw new Error(`向量缺用例 ${n}`);
  return hit[0];
};

const tol = (b: number) => 1e-9 * Math.max(1, Math.abs(b));
const near = (a: number, b: number) =>
  expect(Math.abs(a - b)).toBeLessThanOrEqual(tol(b));

describe.each(CASES)("$name", (c) => {
  const cfg: PoolConfig = { rates: c.rates, pity: c.pity };
  const co = composite(cfg);
  const ec = economics({ ...cfg, cost: c.cost });

  it("composite 与向量逐字段一致（1e-9）", () => {
    expect(co.degenerate).toBe(c.expectedComposite.degenerate);
    near(co.goldBase, c.expectedComposite.goldBase);
    near(co.goldComp, c.expectedComposite.goldComp);
    near(co.cycle, c.expectedComposite.cycle);
    near(co.hardProb, c.expectedComposite.hardProb);
    near(co.upRate, c.expectedComposite.upRate);
    near(co.upShare, c.expectedComposite.upShare);
    near(co.evComp, c.expectedComposite.evComp);
    c.expectedComposite.base.forEach((e, i) => near(co.base[i] ?? NaN, e));
    c.expectedComposite.comp.forEach((e, i) => near(co.comp[i] ?? NaN, e));
  });

  it("质量守恒：Σ comp == 1（1e-9）—— 公示页概率列的完整性", () => {
    const sum = co.comp.reduce((a, b) => a + b, 0);
    expect(Math.abs(sum - 1)).toBeLessThanOrEqual(1e-9);
  });

  it("economics 与向量一致；守卫口径 ratioWorst 取 max(新手, 毕业)", () => {
    const e = c.expectedEconomics;
    near(ec.sv, e.sv);
    near(ec.evNew, e.evNew);
    near(ec.evEnd, e.evEnd);
    near(ec.missRate, e.missRate);
    near(ec.shardRate, e.shardRate);
    near(ec.cardRate, e.cardRate);
    near(ec.ratioNew, e.ratioNew);
    near(ec.ratioEnd, e.ratioEnd);
    near(ec.ratioWorst, e.ratioWorst);
    expect(ec.ratioWorst)
      .toBe(Math.max(ec.ratioNew, ec.ratioEnd));
    expect(ec.svFrom).toEqual(e.svFrom);
  });
});

describe("经济红线（方案 §1）", () => {
  it("守卫必须会红：压低合成价 ⇒ ratioWorst > 1（后门用例 202.78%）", () => {
    const back = byName("backdoor_lr_synth_800");
    const ec = economics({
      rates: back.rates, pity: back.pity, cost: back.cost,
    });
    expect(ec.ratioWorst).toBeGreaterThan(1);
    near(ec.sv, back.expectedEconomics.sv);
  });

  it("demo 池的守卫口径应贴 96.76% —— 与样张实测一致", () => {
    const demo = byName("demo_pool");
    // 96.76% 是样张/文档的 2 位小数口径，只约束到最后一位的舍入
    expect(Math.abs(demo.expectedEconomics.ratioWorst - 0.9676))
      .toBeLessThan(5e-5);
  });
});

describe("simulate（mulberry32 纯整数流，跨语言逐位）", () => {
  const c = byName("mc_200k_seed42") as VectorCase & {
    simulate: NonNullable<VectorCase["simulate"]>;
  };
  const s = simulate(
    { rates: c.rates, pity: c.pity }, c.simulate.n, c.simulate.seed);

  it("整数计数与向量逐位一致", () => {
    expect(s.gold).toBe(c.simulate.gold);
    expect(s.hardHits).toBe(c.simulate.hardHits);
    expect(s.tenWin).toBe(c.simulate.tenWin);
    expect(s.tenTot).toBe(c.simulate.tenTot);
    expect(s.maxGap).toBe(c.simulate.maxGap);
    s.counts.forEach((v, i) => expect(v).toBe(c.simulate!.counts[i]));
  });

  it("MC 出金率落在综合概率抽样误差内（|差| < 0.5pp）", () => {
    const dp = composite({ rates: c.rates, pity: c.pity });
    expect(Math.abs(s.gold / s.N - dp.goldComp)).toBeLessThan(0.005);
    near(s.evSim, c.simulate.evSim);
  });
});

describe("isGold（gold_rank ≥ 3，方案 §0226）", () => {
  it("SSR/UR/LR 出金；R/SR/SHARD/MISS 不出", () => {
    expect(isGold("R")).toBe(false);
    expect(isGold("SR")).toBe(false);
    expect(isGold("SSR")).toBe(true);
    expect(isGold("UR")).toBe(true);
    expect(isGold("LR")).toBe(true);
    expect(isGold("SHARD")).toBe(false);
    expect(isGold("MISS")).toBe(false);
  });
});
