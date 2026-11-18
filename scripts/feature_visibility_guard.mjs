#!/usr/bin/env node
// feature_visibility_guard.mjs —— 功能可见性门禁（0498，产品体检 A1 根治）
//
// 背景：「有后端无前端」断档三次发生（契约三端点 0424 上线到 0498 才有 UI、
// 热力图 heatmap 字段下发了没人消费、发布者分成只有流水标签）。CI 层面
// 没有任何东西拦得住「后端路由加了、前端零调用」。
//
// 口径：登记制清单（scripts/feature_visibility.json）——每条 = 用户面后端
// 端点前缀 + 前端必须存在的消费证据（对 apps/web 的 api.get/post 调用串做
// 子串匹配）。新端点想豁免须在清单里登记 "consumer": null + reason。
// 方向刻意是「端点 → 前端」单向：前端调不存在的端点由 e2e/类型层管。

import { readFileSync, readdirSync, statSync } from "node:fs";
import { join, dirname } from "node:path";
import { fileURLToPath } from "node:url";
import { createHash } from "node:crypto";

const root = join(dirname(fileURLToPath(import.meta.url)), "..");
const REGISTRY = join(root, "scripts", "feature_visibility.json");

function fail(msg) {
  console.error(`[feature-visibility] ${msg}`);
  process.exit(1);
}

const registry = JSON.parse(readFileSync(REGISTRY, "utf8"));

// —— 收集前端源码里的 API 调用串（粗扫，子串匹配足够）——
function walk(dir, out) {
  for (const name of readdirSync(dir)) {
    if (name === "node_modules" || name === ".next") continue;
    const p = join(dir, name);
    const st = statSync(p);
    if (st.isDirectory()) walk(p, out);
    else if (/\.(tsx?|jsx?)$/.test(name)) out.push(p);
  }
  return out;
}
const webFiles = walk(join(root, "apps", "web"), []);
const haystack = webFiles
  .map((f) => {
    try {
      return readFileSync(f, "utf8");
    } catch {
      return "";
    }
  })
  .join("\n");

let bad = 0;
for (const item of registry.required) {
  if (item.consumer === null) {
    if (!item.reason) fail(`${item.endpoint}: 豁免缺 reason`);
    continue;
  }
  if (!haystack.includes(item.consumer)) {
    console.error(
      `[feature-visibility] 后端端点 ${item.endpoint} 前端无消费证据` +
        `（期望 "${item.consumer}"）——「有后端无前端」断档或漏改清单`,
    );
    bad++;
  }
}
if (bad > 0) fail(`${bad} 个端点无前端消费`);

console.log(
  `[feature-visibility] ${registry.required.length} 条登记全过（前端消费证据齐全）`,
);
