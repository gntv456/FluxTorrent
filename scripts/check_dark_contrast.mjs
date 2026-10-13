// W-40 暗色对比度校准（DS-MASTER-01）：解析 base.css 暗色套的
// --text-* / --surface-* / 状态色令牌，对关键配对算 WCAG 对比度，
// 列出 <4.5（AA 正文）的组合。一次性脚本，结果记 docs/design/metrics-log.md。
// 用法：node scripts/check_dark_contrast.mjs
import { readFileSync } from "node:fs";
import path from "node:path";

const css = readFileSync(
  path.resolve(import.meta.dirname, "..", "apps/web/app/styles/base.css"),
  "utf8",
);

// 取某选择器块（到下一个顶级 } 前）里的变量表
function blockVars(selector) {
  const start = css.indexOf(selector);
  if (start < 0) throw new Error(`block not found: ${selector}`);
  const body = css.slice(start, css.indexOf("}", start));
  const vars = {};
  for (const m of body.matchAll(/(--[a-zA-Z0-9-]+)\s*:\s*([^;]+);/g)) {
    vars[m[1]] = m[2].trim();
  }
  return vars;
}

const light = blockVars('[data-theme="baozi"] {');
const dark = blockVars('[data-theme="baozi-night"] {');

function resolve(vars, name, seen = new Set()) {
  let v = vars[name] ?? light[name];
  if (v === undefined) return null;
  v = v.trim();
  if (v.startsWith("var(")) {
    const inner = v.slice(4, -1).split(",")[0].trim();
    if (seen.has(inner)) return null;
    seen.add(inner);
    return resolve(vars, inner, seen);
  }
  const m = v.match(/^#([0-9a-fA-F]{6})$/);
  return m ? m[1] : null; // 非 6 位 hex（渐变/rgba）不参与本轮
}

function lum(hex) {
  const c = [0, 2, 4].map((i) => parseInt(hex.slice(i, i + 2), 16) / 255).map(
    (x) => (x <= 0.03928 ? x / 12.92 : ((x + 0.055) / 1.055) ** 2.4),
  );
  return 0.2126 * c[0] + 0.7152 * c[1] + 0.0722 * c[2];
}

function ratio(fg, bg) {
  const [l1, l2] = [lum(fg), lum(bg)].sort((a, b) => b - a);
  return (l1 + 0.05) / (l2 + 0.05);
}

// 关键配对（正文/标题/次要/微弱文本 × 页面/卡片/沉降面/悬停面 + 状态徽章）
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
  ["信息 sky/卡片", "--sky", "--surface-card"],
  ["表格正文 body-deep/行底", "--text-body-deep", "--surface-card"],
  ["表头 brand/表头底", "--text-brand", "--head-a"],
];

for (const [themeName, vars] of [["light", light], ["dark", dark]]) {
  console.log(`\n== ${themeName} ==`);
  for (const [label, fgName, bgName] of PAIRS) {
    const fg = resolve(vars, fgName);
    const bg = resolve(vars, bgName);
    if (!fg || !bg) {
      console.log(`  --      ${label}（非纯 hex，跳过）`);
      continue;
    }
    const r = ratio(fg, bg);
    const mark = r >= 4.5 ? "  " : r >= 3 ? "AA-L" : "!!";
    console.log(
      `  ${r.toFixed(2).padStart(5)} ${mark}  ${label}  ` +
        `#${fg} on #${bg}`,
    );
  }
}
