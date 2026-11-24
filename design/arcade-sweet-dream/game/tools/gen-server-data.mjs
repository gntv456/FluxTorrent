#!/usr/bin/env node
/**
 * 甜梦钓场 · 客户端配置 → 服务端 data.json
 *
 * 单一数据源是 design/.../game/js/data.js；Rust 侧 include_str!("data.json")。
 * 改数值只改 data.js，然后跑：
 *   node design/arcade-sweet-dream/game/tools/gen-server-data.mjs
 * 再 cargo test -p flux-api fishing_pond 确认口径没漂。
 */
import fs from "node:fs";
import path from "node:path";
import vm from "node:vm";
import { fileURLToPath } from "node:url";

const here = path.dirname(fileURLToPath(import.meta.url));
const gameDir = path.resolve(here, "..");
const repoRoot = path.resolve(gameDir, "../../..");
const srcPath = path.join(gameDir, "js/data.js");
const outPath = path.join(repoRoot, "apps/api/src/games_http/fishing_pond/data.json");

const src = fs.readFileSync(srcPath, "utf8");
const ctx = { window: {}, Date, Math, console };
vm.createContext(ctx);
vm.runInContext(src, ctx);
const D = ctx.window.FTData;
if (!D) throw new Error("data.js 未导出 window.FTData");

const out = {
  rarity: D.RARITY,
  rarityOrder: D.RARITY_ORDER,
  baits: D.BAITS,
  releaseLuck: D.RELEASE_LUCK,
  scenes: D.SCENES.map((s) => ({
    id: s.id,
    name: s.name,
    conds: s.conds,
    price: s.price,
    desc: s.desc,
  })),
  items: D.ITEMS,
  sets: D.SETS,
  fish: D.FISH,
  const: D.CONST,
  ach: D.ACH,
  friends: D.FRIENDS,
  rivals: D.RIVALS,
  rankRewards: D.RANK_REWARDS,
};

// 最小完整性校验：数量与类型错了宁可不写，避免服务端 include_str 到半成品
const assert = (ok, msg) => {
  if (!ok) throw new Error("数据校验失败：" + msg);
};
assert(Object.keys(out.rarity).length >= 4, "稀有度档位不足");
assert(out.scenes.length >= 4, "场景不足");
assert(out.fish.length >= 30, "鱼种数量异常");
assert(out.items.rod.length >= 4 && out.items.bait.length >= 4, "渔具分类为空");
assert(out.sets.length >= 4, "套装不足");
assert(out.ach.length >= 10, "成就不足");
assert(typeof out.const.staminaMax === "number", "CONST 字段缺失");
assert(out.baits.length >= 5 && out.baits[0].price === 0, "打窝饵表异常（首档须免费）");
assert(Object.keys(out.releaseLuck).length === 5, "放生渔运表异常");

fs.writeFileSync(outPath, JSON.stringify(out, null, 1) + "\n");
const size = fs.statSync(outPath).size;
console.log(`wrote ${path.relative(repoRoot, outPath)} · ${size} bytes · ${out.fish.length} fish / ${out.scenes.length} scenes`);
