/**
 * 触摸目标门禁（UI/UX 审查 2026-11-13 新增）。
 *
 * 项目自定 --tap-min: 44px（apps/web/app/styles/base.css），但历史上
 * 大量组件写死 28-36px 的 min-h / h / w，375 视口下命中区过小。
 * 本门禁把「小热区 class 总出现次数」登记进 baseline，只许减不许增，
 * 与 line_limit_guard / ui_design_guard 同一套棘轮策略。
 *
 * 用法：node scripts/check_tap_target.mjs [--update]
 * 退出码：0=未恶化 1=新增/增多   baseline: tap_target_baseline.json
 */
import { readFileSync, writeFileSync, existsSync } from "node:fs";
import { globCompat as globSync } from "./lib/glob_compat.mjs";
import path from "node:path";

const ROOT = path.resolve(import.meta.dirname, "..");
const BASELINE = path.resolve(import.meta.dirname, "tap_target_baseline.json");
const UPDATE = process.argv.includes("--update");
const files = globSync(
  "apps/web/{app,components,lib}/**/*.tsx",
  { cwd: ROOT },
).sort();

// 28/32/36/40px 的显式像素类与 Tailwind h-7..h-9 档
const SMALL = new RegExp(
  "(?:(?:min-h|min-w|h|w)-\\[(?:2[0-9]|3[0-9]|4[0-3])px\\])" +
    "|\\b(?:h|w)-(?:7|8|9)\\b",
  "g",
);

let total = 0;
const byFile = [];
for (const rel of files) {
  const src = readFileSync(path.join(ROOT, rel), "utf8");
  const hits = src.match(SMALL);
  if (!hits) continue;
  total += hits.length;
  byFile.push({ file: rel, count: hits.length });
}
byFile.sort((a, b) => b.count - a.count);

const prev = existsSync(BASELINE)
  ? JSON.parse(readFileSync(BASELINE, "utf8"))
  : null;
console.log("== 触摸目标（<44px 显式小尺寸 class）==");
console.log("  命中 " + total + " 处 / " + byFile.length + " 文件");
for (const x of byFile.slice(0, 8)) {
  console.log("    " + String(x.count).padStart(4) + "  " + x.file);
}

let exitCode = 0;
if (prev && total > (prev.total ?? 0)) {
  exitCode = 1;
  console.error(
    "  FAIL：" + total + " > 基线 " + prev.total +
      "（新增小热区 " + (total - prev.total) + " 处）",
  );
} else if (prev) {
  console.log("  OK：未恶化（基线 " + prev.total + "）");
} else {
  console.log("  首次运行：请确认数值后用 --update 登记基线");
}

if (UPDATE) {
  writeFileSync(
    BASELINE,
    JSON.stringify({ total, files: byFile.length }, null, 2) + "\n",
  );
  console.log("\nbaseline 已更新 -> scripts/tap_target_baseline.json");
}
process.exit(exitCode);
