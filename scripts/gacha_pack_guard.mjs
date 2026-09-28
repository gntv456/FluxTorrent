#!/usr/bin/env node
// gacha_pack_guard.mjs —— 抽卡包载荷守卫（方案 §6.2）
//
// 判据（样张阶段的直接教训）：
//   1) 颜色字面量**只允许出现在 rarities 段**——解析后走树定位，不按行
//      grep（按行 grep 曾假红 10 条、漏排含 "hi" 的行）；
//   2) 卡键白名单：key/name/rarity/section_kind/entity_ref——包内多出的
//      键意味着站长侧词表被冻结成代码，直接红；
//   3) 载荷自带经济断言：rates/pity/cost 在包内时用 packages/gacha-math
//      重算综合返还率，max(新手, 毕业) > 100% → 红（与 gacha_math_guard
//      同一口径；守卫自检必须证明三种红都抓得住，抓不住即守卫损坏）。
//
// 真库可选：包 apply 台账表（0223 site_pack_applies 先例，G31-C 落地）
// 未建/连不上 flux-postgres → 跳过并说明，不算失败；自检仍然生效。
//
// 用法：node scripts/gacha_pack_guard.mjs
// 退出码：0 = 自检过且真库绿或跳过；1 = 红。
import path from "node:path";
import { pathToFileURL } from "node:url";
import { execFileSync } from "node:child_process";

const ROOT = path.resolve(import.meta.dirname, "..");
const MATH_TS = path.join(ROOT, "packages/gacha-math/src/index.ts");

const COLOR = /#[0-9a-fA-F]{3,8}\b|\brgba?\(|\bhsla?\(/;
const CARD_KEYS = new Set([
  "key", "name", "rarity", "section_kind", "entity_ref"]);

function walk(node, section, p, bad) {
  if (typeof node === "string") {
    if (COLOR.test(node) && section !== "rarities") {
      bad.push(`${p}: 颜色字面量越界（只允许 rarities 段）`);
    }
    return;
  }
  if (Array.isArray(node)) {
    node.forEach((x, i) => walk(x, section, `${p}[${i}]`, bad));
    return;
  }
  if (node && typeof node === "object") {
    if (section === "cards" && /^cards\[\d+\]$/.test(p)) {
      const extra = Object.keys(node).filter((k) => !CARD_KEYS.has(k));
      if (extra.length) {
        bad.push(`${p}: 卡键越界 [${extra.join(",")}]（白名单外）`);
      }
    }
    for (const [k, v] of Object.entries(node)) {
      walk(v, section ?? k, p ? `${p}.${k}` : k, bad);
    }
  }
}

function packEconomics(math, payload) {
  const banners = Array.isArray(payload.banners) ? payload.banners : [];
  const rates = payload.rates && typeof payload.rates === "object"
    ? payload.rates : {};
  const out = [];
  for (const b of banners) {
    const rows = rates[b.key];
    if (!Array.isArray(rows) || !rows.length) continue;
    const ec = math.economics({
      rates: rows,
      pity: {
        soft: +b.pity_soft || 0,
        hard: +b.pity_hard || 1,
        ramp: +b.pity_ramp || 0,
      },
      cost: +b.ticket_cost || 1,
    });
    out.push({ key: b.key, ec });
  }
  return out;
}

const math = await import(pathToFileURL(MATH_TS).href);

/* ── 自检（必跑，零依赖）：绿必须绿，三种红必须全被抓 ─────────────────── */
const base = {
  rarities: [
    { key: "R", label: "R", gold_rank: 1, frame: "#5f86c2",
      hi: "#a9c9f2" },
    { key: "SSR", label: "SSR", gold_rank: 3, frame: "#c9a24a",
      hi: "#f6e3a8" },
  ],
  cards: [
    { key: "c1", name: "样本卡", rarity: "R",
      section_kind: "torrent", entity_ref: "E-1042" },
  ],
  banners: [
    { key: "std", ticket_cost: 25, pity_soft: 50, pity_hard: 65,
      pity_ramp: 5 },
  ],
  rates: {
    std: [
      { r: "MISS", type: "miss", w: 10, value: 0 },
      { r: "R", type: "card", w: 90, value: 8, dupe: 8, synth: 600,
        lvMax: 5, lvStep: 12, lvGain: 0.2 },
    ],
  },
};

const bad = [];
walk(structuredClone(base), null, "", bad);
if (bad.length) {
  console.error(`自检失败：干净载荷被误报 ${bad.length} 条：`);
  for (const b of bad) console.error("  " + b);
  process.exit(1);
}
for (const { key, ec } of packEconomics(math, base)) {
  if (!(ec.ratioWorst < 1)) {
    console.error(`自检失败：干净载荷 ${key} 竟然红 ` +
      `(${(ec.ratioWorst * 100).toFixed(2)}%)`);
    process.exit(1);
  }
}

const redPayloads = [
  ["颜色进 cards 段", {
    ...structuredClone(base),
    cards: [{ key: "c2", name: "越界#ff0000", rarity: "R",
      section_kind: "torrent", entity_ref: "E-1042" }],
  }, "颜色字面量越界"],
  ["卡键白名单外", {
    ...structuredClone(base),
    cards: [{ key: "c3", name: "多余键", rarity: "R",
      section_kind: "torrent", entity_ref: "E-1042",
      frame: "站长不该配颜色" }],
  }, "卡键越界"],
  ["载荷经济后门", {
    ...structuredClone(base),
    rates: { std: [
      { r: "MISS", type: "miss", w: 10, value: 0 },
      { r: "R", type: "card", w: 90, value: 3000, dupe: 400,
        synth: 800, lvMax: 10, lvStep: 400, lvGain: 0.5 },
    ] },
  }, null],
];

const selfBad = [];
for (const [label, payload, expect] of redPayloads) {
  const hits = [];
  walk(structuredClone(payload), null, "", hits);
  if (expect && !hits.some((h) => h.includes(expect))) {
    selfBad.push(`红用例「${label}」没被抓住（期望含「${expect}」）`);
  }
  if (!expect) {
    const reds = packEconomics(math, payload)
      .filter((x) => x.ec.ratioWorst > 1);
    if (!reds.length) {
      selfBad.push(`红用例「${label}」经济断言没有红——` +
        "不会红的门禁不算门禁");
    }
  }
}
if (selfBad.length) {
  console.error("守卫自检失败（守卫自身损坏）：");
  for (const b of selfBad) console.error("  " + b);
  process.exit(1);
}
console.log("守卫自检：干净载荷绿；颜色越界/卡键越界/经济后门三种红全部抓住");

/* ── 真库（可选）：apply 台账表未建/连不上 → 跳过并说明 ───────────────── */
const note = [];
function dbQuery(sql) {
  return execFileSync("docker",
    ["exec", "flux-postgres", "psql", "-U", "flux", "-d", "fluxtorrent",
     "-tAc", sql], { encoding: "utf8" }).trim();
}

let packTables;
try {
  packTables = dbQuery(
    "SELECT table_name FROM information_schema.tables " +
    "WHERE table_schema='public' AND " +
    "(table_name LIKE '%pack_appl%' OR table_name LIKE 'gacha_%')")
    .split("\n").filter(Boolean);
} catch {
  note.push("DB 侧跳过（连不上 flux-postgres；自检仍然生效）");
  packTables = null;
}

if (packTables && packTables.length === 0) {
  note.push("包 apply 台账表未落（G31-C 未实施）；自检门已过");
} else if (packTables && packTables.length > 0) {
  const t = packTables.find((x) => x.includes("pack_appl"));
  if (!t) {
    note.push("包台账表不在（gacha_* 表存在但 apply 台账未落）；跳过真库");
  } else {
    try {
      // site_pack_applies 真实列（0223）：snapshot/changes/counts，无 payload——
      // 站型包的 gacha 段在 changes 里（apply 台账）；当前无 gacha 包段属预期
      // （G31 包载荷未落），解析逻辑先就位，落了就接管。
      const raw = dbQuery(
        `SELECT COALESCE(changes::text, '{}'), COALESCE(snapshot::text, \
'{}') FROM ${t} WHERE rolled_back_at IS NULL ORDER BY id DESC LIMIT 50`);
      let worst = null;
      for (const line of raw.split("\n").filter(Boolean)) {
        const [changesTxt, snapTxt] = line.split("|");
        for (const txt of [changesTxt, snapTxt]) {
          let payload;
          try {
            payload = JSON.parse(txt);
          } catch {
            continue;
          }
          const hits = [];
          walk(payload, null, "", hits);
          for (const h of hits) bad.push(`${t}: ${h}`);
          for (const { key, ec } of packEconomics(math, payload)) {
            if (ec.ratioWorst > 1) {
              bad.push(`${t}.${key}: 综合返还率 ` +
                `${(ec.ratioWorst * 100).toFixed(2)}% > 100%（倒灌）`);
            }
            if (!worst || ec.ratioWorst > worst) worst = ec.ratioWorst;
          }
        }
      }
      if (bad.length) {
        console.error(`包载荷红 ${bad.length} 条：`);
        for (const b of bad.slice(0, 20)) console.error("  " + b);
        process.exit(1);
      }
      console.log(
        `真库包载荷：50 条内最差口径 ` +
        `${worst == null ? "（无可算池）" : (worst * 100).toFixed(2) + "%"}，绿`);
    } catch (e) {
      note.push("真库比对跳过（载荷列结构与预期不符：" +
        String(e.message).slice(0, 120) + "）");
    }
  }
}

for (const n of note) console.log("说明：" + n);
console.log("gacha_pack_guard：绿");
