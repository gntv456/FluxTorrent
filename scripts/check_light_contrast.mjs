/**
 * 明色主题对比度门禁（UI/UX 审查 2026-11-13 新增）。
 *
 * check_dark_contrast.mjs 已同时测明暗两套，但明色套是「只测不调」的
 * 已知债（docs/design-system.md §7：25 组里 17 组 <AA）。本脚本把明色
 * 债单独拎出来做强制棘轮：不达标组合数只许减不许增，--strict 时
 * 要求清零。与暗色脚本共用同一套 PAIRS / WCAG 算法口径。
 *
 * 用法：node scripts/check_light_contrast.mjs [--update] [--strict]
 * 退出码：0=通过 1=恶化或 strict 下仍有债
 */
import { readFileSync, writeFileSync, existsSync } from "node:fs";
import path from "node:path";

const ROOT = path.resolve(import.meta.dirname, "..");
const BASELINE = path.resolve(
  import.meta.dirname,
  "light_contrast_baseline.json",
);
const UPDATE = process.argv.includes("--update");
const STRICT = process.argv.includes("--strict");
const css = readFileSync(
  path.join(ROOT, "apps/web/app/styles/base.css"),
  "utf8",
);

function blockVars(selector) {
  const start = css.indexOf(selector);
  if (start < 0) throw new Error("block not found: " + selector);
  const body = css.slice(start, css.indexOf("}", start));
  const vars = {};
  for (const m of body.matchAll(/(--[\w-]+)\s*:\s*([^;]+);/g)) {
    vars[m[1]] = m[2].trim();
  }
  return vars;
}
const light = blockVars('[data-theme="brand"] {');
function resolve(name, seen = new Set()) {
  let v = light[name];
  if (!v) return null;
  if (v.startsWith("var(")) {
    const inner = v.slice(4, -1).split(",")[0].trim();
    if (seen.has(inner)) return null;
    seen.add(inner);
    return resolve(inner, seen);
  }
  const m = v.match(/^#([0-9a-fA-F]{6})$/);
  return m ? m[1] : null;
}
function lum(hex) {
  const c = [0, 2, 4]
    .map((i) => parseInt(hex.slice(i, i + 2), 16) / 255)
    .map((x) => (x <= 0.03928 ? x / 12.92 : ((x + 0.055) / 1.055) ** 2.4));
  return 0.2126 * c[0] + 0.7152 * c[1] + 0.0722 * c[2];
}
function ratio(fg, bg) {
  const [a, b] = [lum(fg), lum(bg)].sort((x, y) => y - x);
  return (a + 0.05) / (b + 0.05);
}
// 与 check_dark_contrast.mjs 的 PAIRS 保持同一口径
const PAIRS = [
  ["正文 body/卡片", "--text-body", "--surface-card"],
  ["正文 body/页面", "--text-body", "--surface-page"],
  ["正文 body/沉降面", "--text-body", "--surface-sunken"],
  ["正文 body/悬停面", "--text-body", "--surface-hover"],
  ["正文 body/抬高面", "--text-body", "--surface-raised"],
  ["标题 heading/卡片", "--text-heading", "--surface-card"],
  ["标题 strong/卡片", "--text-strong", "--surface-card"],
  ["次要 muted/卡片", "--text-muted", "--surface-card"],
  ["次要 muted/页面", "--text-muted", "--surface-page"],
  ["次要 muted/沉降面", "--text-muted", "--surface-sunken"],
  ["微弱 faint/卡片", "--text-faint", "--surface-card"],
  ["更微弱 fainter/卡片", "--text-fainter", "--surface-card"],
  ["品牌文本 brand/卡片", "--text-brand", "--surface-card"],
  ["品牌文本 brand/页面", "--text-brand", "--surface-page"],
  ["禁用态 muted/柔和底", "--text-muted", "--muted-soft"],
  ["成功 success/软底", "--success", "--success-soft"],
  ["成功 success/卡片", "--success", "--surface-card"],
  ["警告 warning/软底", "--warning", "--warning-soft"],
  ["警告 warning/卡片", "--warning", "--surface-card"],
  ["危险 danger/软底", "--danger", "--danger-soft"],
  ["危险 danger/卡片", "--danger", "--surface-card"],
  ["信息 info/卡片", "--info", "--surface-card"],
  // --sky 是装饰色（渐变/边框），正文用 --sky-text（2026-11-13 拆分）
  ["信息 sky 文本/卡片", "--sky-text", "--surface-card"],
  ["表格正文 body-deep/行底", "--text-body-deep", "--surface-card"],
  ["表头 brand/表头底", "--text-brand", "--head-a"],
];

const AA = 4.5;
const debt = [];
console.log("== 明色主题对比度（AA 4.5:1）==");
for (const [label, fgName, bgName] of PAIRS) {
  const fg = resolve(fgName);
  const bg = resolve(bgName);
  if (!fg || !bg) continue;
  const r = ratio(fg, bg);
  console.log("  " + r.toFixed(2).padStart(5) +
    (r >= AA ? " OK  " : " !!  ") + label);
  if (r < AA) debt.push(label);
}
debt.sort();
console.log("  债（<AA）：" + debt.length + "/" + PAIRS.length + " 组");

const prev = existsSync(BASELINE)
  ? JSON.parse(readFileSync(BASELINE, "utf8"))
  : null;
let exitCode = 0;
if (prev) {
  const prevSet = new Set(prev.debt ?? []);
  const added = debt.filter((l) => !prevSet.has(l));
  if (added.length) {
    exitCode = 1;
    console.error("  FAIL：新增不达标组合 " + added.length + " 个");
    for (const l of added) console.error("    " + l);
  } else if (debt.length > (prev.debt ?? []).length) {
    exitCode = 1;
    console.error("  FAIL：不达标组合数比基线增多");
  } else {
    console.log("  OK：未恶化（基线 " + (prev.debt ?? []).length + " 组）");
  }
} else {
  console.log("  首次运行：请确认清单后用 --update 登记基线");
}
if (STRICT && debt.length) {
  exitCode = 1;
  console.error("  FAIL（--strict）：仍有 " + debt.length + " 组 <AA");
}
if (!STRICT && debt.length === 0) {
  console.log("  OK：明色套全部达标");
}
if (UPDATE) {
  writeFileSync(BASELINE, JSON.stringify({ debt }, null, 2) + "\n");
  console.log("\nbaseline 已更新 -> scripts/light_contrast_baseline.json");
}
process.exit(exitCode);
