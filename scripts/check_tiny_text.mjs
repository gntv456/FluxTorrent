/**
 * 正文字号门禁（UI/UX 审查 2026-11-13 新增）。
 *
 * skill ui-ux-pro-max / quick-reference.md 与项目自身审计均把
 * <12px 正文列为反模式（亮色主题下 --text-fainter 2.17:1 更不可读）。
 * 本门禁统计 tsx 的 text-[9|10|10.5|11px] 与 CSS 的
 * font-size: 9|10|10.5|11px，登记 baseline 只许减不许增。
 *
 * 用法：node scripts/check_tiny_text.mjs [--update]
 * 退出码：0=未恶化 1=增多   baseline: tiny_text_baseline.json
 */
import { readFileSync, writeFileSync, existsSync } from "node:fs";
import { globCompat as globSync } from "./lib/glob_compat.mjs";
import path from "node:path";

const ROOT = path.resolve(import.meta.dirname, "..");
const BASELINE = path.resolve(import.meta.dirname, "tiny_text_baseline.json");
const UPDATE = process.argv.includes("--update");
const tsx = globSync(
  "apps/web/{app,components,lib}/**/*.tsx",
  { cwd: ROOT },
).sort();
const css = globSync("apps/web/app/**/*.css", { cwd: ROOT }).sort();

const TSX_RE = /text-\[(?:9|10|10\.5|11)px\]/g;
const CSS_RE = /font-size:\s*(?:9|10|10\.5|11)px/g;

let tsxCount = 0;
let cssCount = 0;
for (const rel of tsx) {
  tsxCount += (readFileSync(path.join(ROOT, rel), "utf8")
    .match(TSX_RE) ?? []).length;
}
for (const rel of css) {
  cssCount += (readFileSync(path.join(ROOT, rel), "utf8")
    .match(CSS_RE) ?? []).length;
}
const total = tsxCount + cssCount;
const prev = existsSync(BASELINE)
  ? JSON.parse(readFileSync(BASELINE, "utf8"))
  : null;

console.log("== <12px 正文字号 ==");
console.log("  tsx " + tsxCount + " 处 / css " + cssCount +
  " 处 / 合计 " + total);
let exitCode = 0;
if (prev && total > (prev.total ?? 0)) {
  exitCode = 1;
  console.error(
    "  FAIL：" + total + " > 基线 " + prev.total +
      "（新增 " + (total - prev.total) + " 处）",
  );
} else if (prev) {
  console.log("  OK：未恶化（基线 " + prev.total + "）");
} else {
  console.log("  首次运行：请确认数值后用 --update 登记基线");
}

if (UPDATE) {
  writeFileSync(
    BASELINE,
    JSON.stringify({ total, tsx: tsxCount, css: cssCount }, null, 2) + "\n",
  );
  console.log("\nbaseline 已更新 -> scripts/tiny_text_baseline.json");
}
process.exit(exitCode);
