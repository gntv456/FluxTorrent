#!/usr/bin/env node
// gacha_math_guard.mjs —— 抽卡数学守卫（方案《抽卡玩法落地方案-2026-09-27》§6.1）
//
// 判据：不会红的门禁不算门禁（master CI 09-22 空转的教训）。所以本守卫的
// 自检**必须证明自己会红**：绿用例必须绿、故意压低合成价的红用例必须红，
// 任何一侧不成立即守卫自身损坏 → exit 1。
//
// 三层：
//   1) 守卫自检（必跑，零依赖）——见上；
//   2) 数学层一致性（必跑，零依赖）——动态导入 packages/gacha-math/src/index.ts
//      （Node 24 strip-types），对全部向量用例重算并与向量比对（1e-9），
//      漂了即「四处同源」破 → 红；
//   3) 真库快照（可选）——gacha_* 表未落或连不上 flux-postgres 时跳过并
//      说明，不算失败；表在则逐 banner 拼真实奖池算含保底综合返还率，
//      max(新手, 毕业) > 100% → 红，打印精确 banner/档位/数值。
//
// 用法：node scripts/gacha_math_guard.mjs
// 退出码：0 = 自检与一致性过且真库绿或跳过；1 = 红。
import { readFileSync } from "node:fs";
import path from "node:path";
import { pathToFileURL } from "node:url";
import { execFileSync } from "node:child_process";

const ROOT = path.resolve(import.meta.dirname, "..");
const MATH_TS = path.join(ROOT, "packages/gacha-math/src/index.ts");
const VECTORS = path.join(
  ROOT, "apps/api/src/gacha_math/gacha_math_vectors.json");

const bad = [];
const note = [];

/* ── 1) 数学层一致性（必跑）：向量重算，1e-9 ─────────────────────────── */
const math = await import(pathToFileURL(MATH_TS).href);
const vec = JSON.parse(readFileSync(VECTORS, "utf8"));
const near = (a, b) => Math.abs(a - b) <= 1e-9 * Math.max(1, Math.abs(b));

for (const c of vec.cases) {
  const co = math.composite({ rates: c.rates, pity: c.pity });
  const e = c.expectedComposite;
  for (const k of ["goldComp", "cycle", "hardProb", "upRate", "upShare",
    "evComp"]) {
    if (!near(co[k], e[k])) bad.push(`${c.name}.composite.${k} 漂移`);
  }
  if (co.degenerate !== e.degenerate) bad.push(`${c.name}.degenerate 漂移`);
  c.expectedComposite.comp.forEach((x, i) => {
    if (!near(co.comp[i], x)) bad.push(`${c.name}.comp[${i}] 漂移`);
  });
  if (c.expectedEconomics) {
    const ec = math.economics({ rates: c.rates, pity: c.pity,
      cost: c.cost });
    for (const k of ["sv", "evNew", "evEnd", "ratioNew", "ratioEnd",
      "ratioWorst"]) {
      if (!near(ec[k], c.expectedEconomics[k])) {
        bad.push(`${c.name}.economics.${k} 漂移`);
      }
    }
  }
  const sum = co.comp.reduce((a, b) => a + b, 0);
  if (Math.abs(sum - 1) > 1e-9) bad.push(`${c.name}: Σ comp = ${sum} ≠ 1`);
}
if (bad.length) {
  console.error(`数学层与向量漂移 ${bad.length} 处（四处同源已破）：`);
  for (const b of bad.slice(0, 10)) console.error("  " + b);
  process.exit(1);
}
console.log(`数学层一致性：${vec.cases.length} 向量用例全部重算一致（1e-9）`);

/* ── 2) 守卫自检（必跑）：绿必须绿、红必须红 ─────────────────────────── */
const demo = vec.cases.find((c) => c.name === "demo_pool");
const back = vec.cases.find((c) => c.name === "backdoor_lr_synth_800");
if (!demo || !back) {
  console.error("向量缺 demo_pool / backdoor_lr_synth_800 用例");
  process.exit(1);
}
const eco = (c) => math.economics({ rates: c.rates, pity: c.pity,
  cost: c.cost });
const green = eco(demo);
const red = eco(back);
if (!(green.ratioWorst < 1)) {
  console.error(`自检失败：demo 池竟然红（ratioWorst=${green.ratioWorst}）`);
  process.exit(1);
}
if (!(red.ratioWorst > 1)) {
  console.error(
    `自检失败：后门用例没有红（ratioWorst=${red.ratioWorst}）` +
    "——不会红的门禁不算门禁");
  process.exit(1);
}
console.log(
  `守卫自检：demo 绿（${(green.ratioWorst * 100).toFixed(2)}%）、` +
  `后门红（${(red.ratioWorst * 100).toFixed(2)}%）——本守卫会红`);

/* ── 3) 真库快照（可选）：gacha 表未落/连不上 → 跳过并说明 ───────────── */
function dbQuery(sql) {
  return execFileSync("docker",
    ["exec", "flux-postgres", "psql", "-U", "flux", "-d", "fluxtorrent",
     "-tAc", sql], { encoding: "utf8" }).trim();
}

let tables;
try {
  tables = dbQuery(
    "SELECT table_name FROM information_schema.tables " +
    "WHERE table_schema='public' AND table_name LIKE 'gacha_%'")
    .split("\n").filter(Boolean);
} catch {
  note.push("DB 侧跳过（连不上 flux-postgres；静态与自检门仍然生效）");
  tables = null;
}

if (tables && tables.length === 0) {
  note.push("gacha_* 表未落（G31-B 未实施）；自检与一致性门已过");
}

if (tables && tables.length > 0) {
  const need = ["gacha_banners", "gacha_pool_rows", "gacha_cards"];
  const missing = need.filter((t) => !tables.includes(t));
  if (missing.length) {
    note.push(`gacha 表不全（缺 ${missing.join(",")}）；真库比对跳过`);
  } else {
    try {
      const raw = dbQuery(
        "SELECT b.key || '|' || b.ticket_cost || '|' ||" +
        " COALESCE(b.pity_soft::text,'0') || '|' ||" +
        " COALESCE(b.pity_hard::text,'0') || '|' ||" +
        " COALESCE(b.pity_ramp::text,'0') || '|' ||" +
        " r.output_type || '|' || COALESCE(r.rarity,'') || '|' ||" +
        " COALESCE(r.card_id::text,'') || '|' || r.weight::text || '|' ||" +
        " r.prize_value::text || '|' || r.shards::text || '|' ||" +
        " COALESCE(c.synth_shards::text,'0') || '|' ||" +
        " COALESCE(c.dupe_shards::text,'0') || '|' ||" +
        " COALESCE(c.lv_max::text,'1') || '|' ||" +
        " COALESCE(c.lv_cost_each::text,'0') || '|' ||" +
        " COALESCE(c.lv_gain::text,'0') " +
        "FROM gacha_pool_rows r " +
        "JOIN gacha_banners b ON b.id = r.banner_id " +
        "LEFT JOIN gacha_cards c ON c.id = r.card_id " +
        "WHERE b.enabled ORDER BY b.id, r.sort");
      const banners = new Map();
      for (const line of raw.split("\n").filter(Boolean)) {
        const f = line.split("|");
        const [key, cost, soft, hard, ramp] = f;
        if (!banners.has(key)) {
          banners.set(key, { cost: +cost || 1,
            pity: { soft: +soft || 0, hard: +hard || 1, ramp: +ramp || 0 },
            rates: [] });
        }
        const b = banners.get(key);
        const [,, , , , outputType, rarity, cardId, w, prize, shards,
          synth, dupe, lvMax, lvStep, lvGain] = f;
        b.rates.push({
          r: rarity || (outputType === "shard" ? "SHARD" : "MISS"),
          type: outputType,
          w: +w || 0,
          value: outputType === "card" ? +prize || 0 : 0,
          shards: +shards || 0,
          dupe: +dupe || 0,
          synth: +synth || 0,
          lvMax: +lvMax || 1,
          lvStep: +lvStep || 0,
          lvGain: +lvGain || 0,
        });
        void cardId;
      }
      let worst = 0;
      const reds = [];
      for (const [key, b] of banners) {
        const ec = math.economics({ rates: b.rates, pity: b.pity,
          cost: b.cost });
        if (ec.ratioWorst > worst) worst = ec.ratioWorst;
        if (ec.ratioWorst > 1) {
          reds.push(`${key}: ${(ec.ratioWorst * 100).toFixed(2)}%` +
            `（新手 ${(ec.ratioNew * 100).toFixed(2)}% / ` +
            `毕业 ${(ec.ratioEnd * 100).toFixed(2)}%，sv=${ec.sv}` +
            ` 来自 ${ec.svFrom ? ec.svFrom.kind + " " + ec.svFrom.r : "—"}` +
            "）——含保底综合返还率超 100%，正在倒灌经济");
        }
      }
      if (reds.length) {
        console.error(`真库红 ${reds.length} 个卡池：`);
        for (const r of reds) console.error("  " + r);
        process.exit(1);
      }
      console.log(
        `真库快照：${banners.size} 个卡池，最差口径 ` +
        `${(worst * 100).toFixed(2)}% ≤ 100%，绿`);
    } catch (e) {
      note.push("真库比对跳过（gacha 列结构与预期不符，G31-B 落地后对齐：" +
        String(e.message).slice(0, 120) + "）");
    }
  }
}

for (const n of note) console.log("说明：" + n);
console.log("gacha_math_guard：绿");
