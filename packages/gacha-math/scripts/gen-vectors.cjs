/* gen-vectors.cjs —— 以未入库样张 .workbuddy/gacha-shared.js 为 oracle：
   1) 逐位比对 packages/gacha-math/src/index.ts（node 原生 strip-types 直接跑 TS）与样张；
   2) 任一数字不一致即退出非零 —— 向量不生成，防止带病镜像；
   3) 全部一致才把样张输出写成 apps/api/src/gacha_math_vectors.json（两侧镜像用例共用）。
   重生成：node packages/gacha-math/scripts/gen-vectors.cjs （需样张在场，gitignored）。 */
'use strict';
const fs = require('node:fs');
const path = require('node:path');
const { pathToFileURL } = require('node:url');

const ROOT = path.resolve(__dirname, '..', '..', '..');
const SAMPLE = process.env.GACHA_SAMPLE || path.join(ROOT, '.workbuddy', 'gacha-shared.js');
const PORT = path.join(ROOT, 'packages', 'gacha-math', 'src', 'index.ts');
const OUT = path.join(ROOT, 'apps', 'api', 'src', 'gacha_math',
  'gacha_math_vectors.json');

/* 样张模拟页的默认池（gacha-sim-demo.html）与其余边界构造。 */
const DEMO = [
  { r: 'MISS', type: 'miss', w: 10, value: 0 },
  { r: 'SHARD', type: 'shard', w: 12, value: 0, shards: 15 },
  { r: 'R', type: 'card', w: 50, value: 8, dupe: 8, synth: 60, lvMax: 5, lvStep: 12, lvGain: .20 },
  { r: 'SR', type: 'card', w: 22, value: 20, dupe: 20, synth: 180, lvMax: 6, lvStep: 22, lvGain: .25 },
  { r: 'SSR', type: 'card', w: 2.0, value: 200, dupe: 60, synth: 520, lvMax: 10, lvStep: 40, lvGain: .50 },
  { r: 'UR', type: 'card', w: 0.4, value: 800, dupe: 160, synth: 1400, lvMax: 10, lvStep: 120, lvGain: .50 },
  { r: 'LR', type: 'card', w: 0.1, value: 3000, dupe: 400, synth: 3200, lvMax: 10, lvStep: 400, lvGain: .50 },
];
const PITY = { soft: 50, hard: 65, ramp: 5 };
const NOGOLD = DEMO.filter((x) => x.type !== 'card' || x.r === 'R' || x.r === 'SR');
const ALLGOLD = DEMO.filter((x) => x.r === 'SSR' || x.r === 'UR' || x.r === 'LR');

const CASES = [
  { name: 'demo_pool', rates: DEMO, pity: PITY, cost: 25 },
  { name: 'backdoor_lr_synth_800', rates: DEMO.map((x) => x.r === 'LR' ? { ...x, synth: 800 } : x), pity: PITY, cost: 25 },
  { name: 'up_mix', rates: DEMO, pity: { ...PITY, upRatio: 0.5, guaranteeUp: true }, cost: 25 },
  { name: 'degenerate_no_gold', rates: NOGOLD, pity: PITY, cost: 25 },
  { name: 'degenerate_all_gold', rates: ALLGOLD, pity: PITY, cost: 25 },
  { name: 'hard_pity_one', rates: DEMO, pity: { soft: 50, hard: 1, ramp: 5 }, cost: 25 },
  { name: 'no_pity', rates: DEMO, pity: { soft: 0, hard: 65, ramp: 0 }, cost: 25 },
  { name: 'mc_200k_seed42', rates: DEMO, pity: PITY, cost: 25, sim: { N: 200000, seed: 42 } },
];

const svFrom = (s) => {
  if (typeof s !== 'string') return null;
  if (s.startsWith('合成 ')) return { kind: 'synth', r: s.slice(3) };
  if (s.startsWith('升级 ')) return { kind: 'level', r: s.slice(3) };
  return null;
};

/* 逐位（===）比较；数组同长同序。NaN 视为相等。 */
function bitEq(a, b, where, errs) {
  if (typeof a === 'number' && typeof b === 'number') {
    if (a === b || (Number.isNaN(a) && Number.isNaN(b))) return;
    errs.push(`${where}: 样张 ${a} vs 移植 ${b}`);
    return;
  }
  if (Array.isArray(a) && Array.isArray(b)) {
    if (a.length !== b.length) { errs.push(`${where}: 长度 ${a.length} vs ${b.length}`); return; }
    a.forEach((x, i) => bitEq(x, b[i], `${where}[${i}]`, errs));
    return;
  }
  if (a && b && typeof a === 'object' && typeof b === 'object'
    && !Array.isArray(a) && !Array.isArray(b)) {
    const ka = Object.keys(a).sort(), kb = Object.keys(b).sort();
    if (ka.length !== kb.length || ka.some((k, i) => k !== kb[i])) {
      errs.push(`${where}: 键 ${JSON.stringify(ka)} vs ${JSON.stringify(kb)}`);
      return;
    }
    for (const k of ka) bitEq(a[k], b[k], `${where}.${k}`, errs);
    return;
  }
  if (a === b) return;
  errs.push(`${where}: 样张 ${JSON.stringify(a)} vs 移植 ${JSON.stringify(b)}`);
}

(async () => {
  globalThis.window = {};
  require(SAMPLE);
  const G = globalThis.window.Gacha;
  if (!G) { console.error('样张加载失败'); process.exit(1); }
  const P = await import(pathToFileURL(PORT).href);

  const out = [];
  const errs = [];
  for (const c of CASES) {
    const cfg = { rates: c.rates, pity: c.pity };
    const eco = { ...cfg, cost: c.cost };
    const sc = G.composite(cfg), pc = P.composite(cfg);
    for (const k of ['base', 'comp']) bitEq(sc[k], pc[k], `${c.name}.composite.${k}`, errs);
    for (const k of ['goldBase', 'goldComp', 'cycle', 'hardProb', 'upRate', 'upShare', 'evComp']) {
      bitEq(sc[k], pc[k], `${c.name}.composite.${k}`, errs);
    }
    bitEq(sc.degenerate, pc.degenerate, `${c.name}.composite.degenerate`, errs);
    const se = G.economics(eco), pe = P.economics(eco);
    for (const k of ['sv', 'evNew', 'evEnd', 'missRate', 'shardRate', 'cardRate',
      'ratioNew', 'ratioEnd', 'ratioWorst']) bitEq(se[k], pe[k], `${c.name}.economics.${k}`, errs);
    bitEq(svFrom(se.svFrom), pe.svFrom, `${c.name}.economics.svFrom`, errs);
    const comp = sc.comp;
    const sum = comp.reduce((a, b) => a + b, 0);
    if (Math.abs(sum - 1) > 1e-12) errs.push(`${c.name}: Σ comp = ${sum} ≠ 1`);
    const row = {
      name: c.name, rates: c.rates, pity: c.pity, cost: c.cost,
      expectedComposite: {
        base: sc.base, goldBase: sc.goldBase, comp: sc.comp,
        goldComp: sc.goldComp, cycle: sc.cycle, hardProb: sc.hardProb,
        upRate: sc.upRate, upShare: sc.upShare, evComp: sc.evComp,
        degenerate: sc.degenerate,
      },
      expectedEconomics: {
        sv: se.sv, svFrom: svFrom(se.svFrom), evNew: se.evNew, evEnd: se.evEnd,
        missRate: se.missRate, shardRate: se.shardRate, cardRate: se.cardRate,
        cost: se.cost, ratioNew: se.ratioNew, ratioEnd: se.ratioEnd,
        ratioWorst: se.ratioWorst,
      },
    };
    if (c.sim) {
      const ss = G.simulate(cfg, c.sim.N, c.sim.seed);
      const ps = P.simulate(cfg, c.sim.N, c.sim.seed);
      for (const k of ['freq', 'counts']) bitEq(ss[k], ps[k], `${c.name}.simulate.${k}`, errs);
      for (const k of ['gold', 'hardHits', 'tenRate', 'evSim', 'evBase', 'goldBase']) {
        bitEq(ss[k], ps[k], `${c.name}.simulate.${k}`, errs);
      }
      row.simulate = {
        n: c.sim.N, seed: c.sim.seed, gold: ss.gold, hardHits: ss.hardHits,
        tenWin: Math.round(ss.tenRate * Math.floor(c.sim.N / 10)),
        tenTot: Math.floor(c.sim.N / 10),
        maxGap: ss.gaps.length ? Math.max(...ss.gaps) : 0,
        counts: ss.counts, evSim: ss.evSim, evBase: ss.evBase,
      };
      bitEq(row.simulate.tenWin, Math.round(ps.tenRate * Math.floor(c.sim.N / 10)),
        `${c.name}.simulate.tenWin(移植重算)`, errs);
    }
    out.push(row);
  }
  if (errs.length) {
    console.error(`移植与样张不一致 ${errs.length} 处：`);
    for (const e of errs.slice(0, 20)) console.error('  ' + e);
    process.exit(1);
  }
  const vectors = {
    meta: {
      source: '.workbuddy/gacha-shared.js（样张，gitignored，未入库）',
      generated: '2026-09-27',
      tolerance: 'f64 字段 1e-9；simulate 的 gold/hardHits/tenWin/tenTot/counts/maxGap 为整数，逐位相等',
      regenerate: 'node packages/gacha-math/scripts/gen-vectors.cjs',
    },
    cases: out,
  };
  fs.writeFileSync(OUT, JSON.stringify(vectors, null, 2) + '\n');
  const d = out[0], b = out[1], m = out.find((x) => x.simulate);
  console.log('向量已写入', path.relative(ROOT, OUT), `(${out.length} 用例)`);
  console.log('goldComp =', (d.expectedComposite.goldComp * 100).toFixed(4) + '%');
  console.log('ratioWorst demo =', (d.expectedEconomics.ratioWorst * 100).toFixed(2) + '%',
    '| backdoor =', (b.expectedEconomics.ratioWorst * 100).toFixed(2) + '%');
  console.log('MC gold =', (m.simulate.gold / m.simulate.n * 100).toFixed(4) + '%',
    '| DP goldComp =', (m.expectedComposite.goldComp * 100).toFixed(4) + '%',
    '| maxGap =', m.simulate.maxGap, '| hardHits =', m.simulate.hardHits);
})();
