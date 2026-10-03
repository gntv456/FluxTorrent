/**
 * 定向收紧 i18n 基线：只重算指定文件的命中行，其余文件条目原样保留。
 *
 * 为什么不用 `i18n_guard.mjs --update`：那条路会把**所有**文件的当前状态写成基线，
 * 于是一旦并行会话在别处新增了硬编码中文，也会被顺手"洗白"。本脚本只动我改过的
 * 文件，保持基线对他人文件的约束力。
 *
 * 命中判定与 i18n_guard.mjs 的 hitsOf 逐字对齐（同一套正则与跳过规则）。
 *
 * 用法：node scripts/i18n_baseline_touch.mjs <相对 apps/web 的路径>...
 */
import { readFileSync, existsSync, writeFileSync } from "node:fs";
import path from "node:path";

const ROOT = path.resolve(import.meta.dirname, "..", "apps/web");
const BASELINE = path.resolve(import.meta.dirname, "i18n_baseline.json");

function hitsOf(file) {
  const lines = readFileSync(path.join(ROOT, file), "utf-8").split("\n");
  const out = [];
  lines.forEach((l, i) => {
    const t = l.trim();
    if (t.startsWith("//") || t.startsWith("*") || t.startsWith("/*")) return;
    const jsx = l.match(/>\s*([^<>{}\n]*[\u4e00-\u9fff][^<>{}\n]*)\s*</);
    const str = l.match(/["'`]([^"'`\n]*[\u4e00-\u9fff][^"'`\n]*)["'`]/);
    if (jsx || str) out.push(i + 1);
  });
  return out;
}

const targets = process.argv.slice(2).map((p) => p.replace(/\\/g, "/"));
if (targets.length === 0) {
  console.error("用法：node scripts/i18n_baseline_touch.mjs <file>...");
  process.exit(2);
}

const baseline = existsSync(BASELINE)
  ? JSON.parse(readFileSync(BASELINE, "utf-8"))
  : {};

// ⚠️ 基线键统一**正斜杠**（与 i18n_guard.mjs 的归一口径一致；历史上 Windows
// glob 曾产出反斜杠键，已由 normalize_i18n_baseline_keys.py 一次性归一）。
// 传参两种斜杠都接受，落键前归一，避免写出两套重复键。
const keyOf = (f) => f.replace(/\\/g, "/");

let touched = 0;
for (const f of targets) {
  const key = keyOf(f);
  const before = baseline[key];
  const after = hitsOf(f);
  const b = before ? before.length : null;
  baseline[key] = after;
  touched++;
  console.log(
    `  ${key}\n    基线 ${b === null ? "(无)" : b + " 条"} -> ${after.length} 条` +
      (after.length ? `  行号: ${after.join(",")}` : "  (已清零)"),
  );
}

writeFileSync(BASELINE, JSON.stringify(baseline, null, 1));
console.log(`\n已更新 ${touched} 个文件的基线条目（其余文件未改动）`);
