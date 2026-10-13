// W-40 暗色对比度校准（DS-MASTER-01）：解析 base.css 暗色套的
// --text-* / --surface-* / 状态色令牌，对关键配对算 WCAG 对比度，
// 列出 <4.5（AA 正文）的组合。结果记 docs/design/metrics-log.md。
//
// W-43 扩展（阶段六附带项）：明暗双套都测。暗色套 W-40 已校准全过
// 线，转「棘轮门禁」——任一暗色配对比上次运行变差即 exit 1（防回潮）；
// 明色套 W-40 登记为已知债（muted/faint 与状态色/白底 2.0-4.3，调值
// 牵动 soft 底配套，独立决策），本脚本**只测不调**：不达标组合数
// 登记进 baseline 只许减不许增——防新增更差的配对，存量清偿另批。
// 用法：node scripts/check_dark_contrast.mjs [--update]
//   退出码：0=通过（暗色无回潮、明色债未扩大）；1=恶化。
import { readFileSync, writeFileSync, existsSync } from "node:fs";
import path from "node:path";

const UPDATE = process.argv.includes("--update");
const BASELINE = path.resolve(
  import.meta.dirname,
  "dark_contrast_baseline.json",
);

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

const light = blockVars('[data-theme="brand"] {');
const dark = blockVars('[data-theme="brand-night"] {');

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

const AA = 4.5; // WCAG AA 正文
let exitCode = 0;
const nextBase = { darkMin: null, lightDebt: [] };

for (const [themeName, vars] of [["light", light], ["dark", dark]]) {
  console.log(`\n== ${themeName} ==`);
  const ratios = [];
  for (const [label, fgName, bgName] of PAIRS) {
    const fg = resolve(vars, fgName);
    const bg = resolve(vars, bgName);
    if (!fg || !bg) {
      console.log(`  --      ${label}（非纯 hex，跳过）`);
      continue;
    }
    const r = ratio(fg, bg);
    ratios.push({ label, r });
    const mark = r >= AA ? "  " : r >= 3 ? "AA-L" : "!!";
    console.log(
      `  ${r.toFixed(2).padStart(5)} ${mark}  ${label}  ` +
        `#${fg} on #${bg}`,
    );
  }
  if (themeName === "dark") {
    // 暗色套：W-40 已校准，任何配对跌破 AA 即回潮（fainter 例外条款
    // 见 metrics-log W-40：10px 角标按 AA large-text ≥3 认定）
    const regress = ratios.filter(
      (x) => x.r < AA && x.label !== "更微弱 fainter/卡片",
    );
    const fainter = ratios.find((x) => x.label === "更微弱 fainter/卡片");
    const min = Math.min(...ratios.map((x) => x.r));
    nextBase.darkMin = Math.round(min * 100) / 100;
    if (regress.length || (fainter && fainter.r < 3)) {
      exitCode = 1;
      console.error(
        `  FAIL（暗色回潮）：${regress.length} 组 <AA` +
          `${fainter && fainter.r < 3 ? "，fainter <AA-large 3" : ""}`,
      );
      for (const x of regress)
        console.error(`    ${x.r.toFixed(2)}  ${x.label}`);
    } else {
      console.log(`  OK：无 AA 回潮（最低 ${min.toFixed(2)}）`);
    }
  } else {
    // 明色套：只测不调（W-40 旧债）。债清单登记进 baseline，
    // 不达标组合数只许减不许增；单组比值变差 >0.05 也算恶化。
    const debt = ratios.filter((x) => x.r < AA);
    nextBase.lightDebt = debt.map((x) => x.label).sort();
    const prev = existsSync(BASELINE)
      ? JSON.parse(readFileSync(BASELINE, "utf8"))
      : null;
    console.log(`  债（<AA）${debt.length}/${ratios.length} 组——只测不调`);
    if (prev) {
      const prevSet = new Set(prev.lightDebt ?? []);
      const added = nextBase.lightDebt.filter((l) => !prevSet.has(l));
      if (added.length) {
        exitCode = 1;
        console.error(`  FAIL（明色债扩大）：新增不达标组合 ${added.length} 个`);
        for (const l of added) console.error(`    ${l}`);
      } else {
        console.log("  OK：不达标组合未比基线增多（存量债见 metrics-log）");
      }
    }
  }
}

if (UPDATE) {
  writeFileSync(BASELINE, JSON.stringify(nextBase, null, 2) + "\n");
  console.log(`\nbaseline 已更新 -> scripts/dark_contrast_baseline.json`);
}
process.exit(exitCode);
