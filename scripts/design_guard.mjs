// 设计模板门禁（S3 收敛与治理 · 2026-10-16）
// 把 S1（标题级唯一）与 S2（tone 化、禁色类直写）的成果固化成自动检查，
// 防止后续改动回退。三项检查：
//
//   1. h1       页面文件 <h1 字面量 ≤ 1。PageHeader 已把 h1 收进 L2 原语，
//               页面层不该再手写；同一时刻渲染多个 h1 会打乱读屏器大纲。
//               ⚠️ 静态扫描看不穿互斥早返回分支（`if (!x) return <h1/>`），
//               这类会在基线里，由人工确认。
//   2. palette  禁项目语义色类直写（bg-sky / text-mint / bg-sun …）——
//               语义应交给 Badge 的 tone 或 var(--tone-*)，让明暗主题自动适配。
//               这是 S2 Badge 收敛（20 处）的护栏。
//   3. color    hex/rgb 字面量。**只报告不 fail**——命中的绝大多数是正当
//               用法而非待收敛的样式，设成 fail 会大面积误伤：
//                 · 内联 SVG 占位图（<circle fill='rgba(…)'>）4 处
//                 · theme-color meta（浏览器 UI 色只能用字面量）2 处
//                 · 设置项默认值 / 占位色（#FFD700 之类）2 处
//                 · var(--danger,#d33) 这类 fallback 2 处
//                 · 动态色的 || "#fff" 兜底 2 处
//                 · 游戏区（站型包刻意差异，用户已定本轮不动）82 处
//               真·样式硬编码经本轮排查只剩 1 处（论坛楼层作者栏
//               bg-[rgba(255,232,197,0.45)]），已换成 var(--sun-soft)。
//               本项的作用是「让色值保持可见」，不是卡人。
//
// 基线制：存量记 scripts/design_guard_baseline.json，只对**新增**违规 exit 1。
// 用法：node scripts/design_guard.mjs（CI 中作为 web job 一步）
//       node scripts/design_guard.mjs --update          # 全量重算基线
//       node scripts/design_guard.mjs --touch <文件…>  # 定向重算（迁移后用）
// ⚠️ 基线按**行号**记录，文件行增删会让存量被判成新增——迁移某文件后用
// --touch 定向重算，别用 --update（会把并行会话的新违规一并洗白）。
import { readFileSync, existsSync, writeFileSync } from "node:fs";
import { globCompat as globSync } from "./lib/glob_compat.mjs";
import path from "node:path";

const ROOT = path.resolve(import.meta.dirname, "..", "apps/web");
const BASE = path.resolve(
  import.meta.dirname,
  "design_guard_baseline.json",
);

// 项目自定义语义色（base.css 里 --sky/--indigo/--mint/--sun/--coral/
// --tomato/--cloud 的 Tailwind 别名），非 Tailwind 自带色板。
const PALETTE =
  "(sky|indigo|mint|sun|coral|tomato|cloud)";
const COLOR_CLASS = new RegExp(
  `\\b(bg|text|border|fill|stroke|ring|from|to)-${PALETTE}` +
    "(-[0-9]+)?(/[0-9]+)?\\b",
  "g",
);
const RAW_COLOR = /#[0-9a-fA-F]{3,8}\b|\brgba?\(/g;

const files = ["app/**/*.tsx", "components/**/*.tsx"]
  .flatMap((p) => globSync(p, { cwd: ROOT }).map((f) => f.replace(/\\/g, "/")))
  .filter(
    (f) =>
      !f.includes("node_modules") &&
      !f.includes(".next") &&
      !f.includes("/_tmp") &&
      !f.startsWith("components/_"),
  );

/** 文件级命中行号：{h1, palette, color}，忽略注释行（与 i18n_guard 同口径） */
function hitsOf(file) {
  const lines = readFileSync(path.join(ROOT, file), "utf-8").split("\n");
  const out = { h1: [], palette: [], color: [] };
  const isPage = file.endsWith("/page.tsx");
  const h1s = [];
  lines.forEach((l, i) => {
    const t = l.trim();
    if (t.startsWith("//") || t.startsWith("*") || t.startsWith("/*")) return;
    const n = i + 1;
    if (isPage && /<h1[\s>]/.test(l)) h1s.push(n);
    COLOR_CLASS.lastIndex = 0;
    if (COLOR_CLASS.test(l)) out.palette.push(n);
    RAW_COLOR.lastIndex = 0;
    if (RAW_COLOR.test(l)) out.color.push(n);
  });
  // 标题级唯一：只记第 2 个及以后的 <h1>——每页 1 个是合法的，
  // 多出来的才是违规（基线制下，新加第二个 → 出现新行号 → 拦下）。
  out.h1 = h1s.slice(1);
  return out;
}

const KEYS = ["h1", "palette", "color"];
const LABEL = {
  h1: "页面 <h1> 字面量 >1（改用 PageHeader；互斥分支请 --touch 登记）",
  palette: "语义色类直写（改用 Badge tone / var(--tone-*)）",
  color: "色值字面量（v2 转 fail，先报告）",
};
const baseline = existsSync(BASE)
  ? JSON.parse(readFileSync(BASE, "utf-8"))
  : {};

const args = process.argv.slice(2);
const touchAt = args.indexOf("--touch");
if (touchAt >= 0) {
  let n = 0;
  for (const f of args.slice(touchAt + 1)) {
    const key = f.replace(/\\/g, "/");
    baseline[key] = hitsOf(key);
    n++;
  }
  writeFileSync(BASE, JSON.stringify(baseline, null, 1));
  console.log(`baseline touched: ${n} 个文件`);
  process.exit(0);
}
if (args.includes("--update")) {
  const next = {};
  for (const f of files) next[f] = hitsOf(f);
  writeFileSync(BASE, JSON.stringify(next, null, 1));
  console.log(`baseline updated: ${Object.keys(next).length} 文件`);
  process.exit(0);
}

const fresh = { h1: [], palette: [], color: [] };
const stored = { h1: 0, palette: 0, color: 0 };
for (const f of files) {
  const cur = hitsOf(f);
  const was = baseline[f] ?? {};
  for (const k of KEYS) {
    const old = was[k] ?? [];
    stored[k] += old.length;
    const add = cur[k].filter((n) => !old.includes(n));
    if (add.length) fresh[k].push({ file: f, lines: add });
  }
}

let bad = 0;
for (const k of ["h1", "palette"]) {
  if (!fresh[k].length) continue;
  bad++;
  console.error(`design 门禁 · ${k} 新增：${LABEL[k]}`);
  for (const r of fresh[k]) {
    console.error(`  ${r.file}:${r.lines.join(",")}`);
  }
}
if (fresh.color.length) {
  const n = fresh.color.reduce((s, r) => s + r.lines.length, 0);
  console.log(`design 门禁 · color（仅报告，不 fail）：新增 ${n} 处`);
  for (const r of fresh.color) {
    console.log(`  ${r.file}:${r.lines.join(",")}`);
  }
}
if (bad) process.exit(1);
console.log(
  `design 门禁通过（${files.length} 文件；存量 ` +
    `h1 ${stored.h1} / palette ${stored.palette} / color ${stored.color}）`,
);
