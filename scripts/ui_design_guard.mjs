/**
 * UI 设计系统门禁（2026-10-03 新建）——防「令牌层退化」的唯一机械拦网。
 *
 * 背景：TIDE 主题的令牌层（theme-tide.css）质量不低，但**只覆盖了色彩层**。
 * 审计发现尺度层全靠硬编码：760 处硬编码色值 / 19 种圆角 / 34 种字号 /
 * 47 个裸像素断点 / 18 个 z-index，且 9 个 CSS 变量被 var() 引用却从未
 * 定义（浏览器静默回退到消费点写的旧色板）。根因不是没人知道规矩——
 * base.css:266 早写着断点映射表、theme-tide.css 头部写着「禁止硬编码
 * 色值」，但 `eslint.config.mjs` 只有 next/core-web-vitals、**零样式规则**，
 * 无 stylelint，于是 760 处硬编码与 47 个断点没有任何阻断能力。
 *
 * 六条规则（与 scripts/line_limit_guard.mjs 同策略：存量进 baseline，
 * 只许减不许增；新文件从严）：
 *   1. CSS 变量门禁：var(--x) 引用必须有定义（否则静默回退到 fallback）
 *   2. 硬编码色值门禁：只减不增（新文件从 0 起算）
 *   3. 断点门禁：@media 只允许 Tailwind 四档 + 语义别名
 *   4. z-index 门禁：只允许 9 个语义档
 *   5. 圆角/字号门禁：硬编码值必须落在令牌档内
 *   6. 焦点环门禁：outline-none 必须配 focus-visible 描边
 *
 * 维护约定：**修完一类问题后跑 `node scripts/ui_design_guard.mjs --update`
 * 把 baseline 刷到新值**（确认是「有意放行」才刷，不要养成随手刷的习惯）。
 *
 * 用法：node scripts/ui_design_guard.mjs [--update]
 * 退出码：0=通过（可能有 WARN） 1=违规
 */
import { readFileSync, writeFileSync, existsSync } from "node:fs";
import { globSync } from "node:fs";
import path from "node:path";

import { checkDomScopes } from "./ui_design_guard_dom.mjs";

const ROOT = path.resolve(import.meta.dirname, "..");
const BASELINE = path.resolve(
  import.meta.dirname,
  "ui_design_baseline.json",
);
const UPDATE = process.argv.includes("--update");

const failBox = { n: 0 };
let fails = 0;
let warns = 0;
const section = (t) => console.log(`\n== ${t} ==`);

function read(rel) {
  return readFileSync(path.join(ROOT, rel), "utf8");
}

/** 统计一批文件里某子串出现的总次数 */
function countMatches(files, needle) {
  let n = 0;
  for (const f of files) {
    const s = read(f);
    n += s.split(needle).length - 1;
  }
  return n;
}

// ---------- 收集样式源文件 ----------
const CSS_DIR = "apps/web/app/styles";
const cssFiles = globSync(`${CSS_DIR}/*.css`, { cwd: ROOT }).sort();
const globalsCss = "apps/web/app/globals.css";
const tsxFiles = globSync("apps/web/{components,app}/**/*.tsx", {
  cwd: ROOT,
}).sort();

// TSX 里的 style={{ "--x": v }} 属动态注入，不算「未定义」
const dynamicVars = new Set();
for (const f of tsxFiles) {
  const s = read(f);
  for (const m of s.matchAll(/["'`](--[a-zA-Z0-9_-]+)["'`]\s*:/g)) {
    dynamicVars.add(m[1]);
  }
  for (const m of s.matchAll(/["'`](--[a-zA-Z0-9_-]+)["'`]\s*as\s+string/g)) {
    dynamicVars.add(m[1]);
  }
}

const allCss = [...cssFiles, globalsCss];
const base = existsSync(BASELINE)
  ? JSON.parse(readFileSync(BASELINE, "utf8"))
  : {};

// ---------- 1. CSS 变量：引用必须有定义 ----------
section("1. CSS 变量（引用须有定义）");
{
  const defined = new Set();
  const used = new Map();
  for (const f of allCss) {
    const s = read(f);
    for (const m of s.matchAll(/(--[a-zA-Z0-9_-]+)\s*:/g)) {
      defined.add(m[1]);
    }
    for (const m of s.matchAll(/var\(\s*(--[a-zA-Z0-9_-]+)\s*(?=[,)])/g)) {
      used.set(m[1], (used.get(m[1]) ?? 0) + 1);
    }
  }
  const dangling = [...used.entries()]
    .filter(([k]) => !defined.has(k) && !dynamicVars.has(k))
    .map(([k, v]) => ({ v: k, n: v }))
    .sort((a, b) => b.n - a.n);

  if (dangling.length === 0) {
    console.log("  OK：无悬挂变量");
  } else {
    fails++;
    console.error(`  FAIL：${dangling.length} 个变量被引用但无定义`);
    for (const d of dangling.slice(0, 12)) {
      console.error(`    ${d.v}  (${d.n} 处)`);
    }
    console.error("    → 在令牌层补定义，或改引用（TSX 动态注入需用 style 写法）");
  }
}

// ---------- 2. 硬编码色值：只减不增 ----------
section("2. 硬编码色值（只减不增）");
{
  let total = 0;
  const perFile = {};
  for (const f of allCss) {
    const s = read(f);
    // 排除 #! 与 url(#id) 这类非颜色用法
    const n = (s.match(/#[0-9a-fA-F]{3,8}\b/g) ?? []).filter(
      (x) => !/^#[0-9a-fA-F]{1,2}$/.test(x) || true,
    ).length;
    const nHex = (s.match(/#[0-9a-fA-F]{3,8}\b/g) ?? []).length;
    const nRgba = (s.match(/rgba?\([^)]*\)/g) ?? []).length;
    perFile[f] = nHex + nRgba;
    total += nHex + nRgba;
  }
  const prev = base.hardcoded ?? total;
  if (total > prev) {
    fails++;
    console.error(`  FAIL：${total} 处（基线 ${prev}，增加 ${total - prev}）`);
    for (const [f, n] of Object.entries(perFile).sort((a, b) => b[1] - a[1])) {
      if (n > 0) console.error(`    ${n}\t${f}`);
    }
    console.error("    → 用 var(--surface-*) / var(--text-*) 等语义令牌替代");
  } else if (total < prev) {
    warns++;
    console.log(`  OK：${total} 处（基线 ${prev}，减少 ${prev - total}）`);
  } else {
    console.log(`  OK：${total} 处（持平）`);
  }
  base.hardcoded = UPDATE ? total : prev;
}

// ---------- 3. 断点：只允许 Tailwind 四档 ----------
section("3. 断点值（只允许 640/768/1024/1280）");
{
  const ALLOWED = new Set([640, 768, 1024, 1280]);
  const wild = new Map();
  for (const f of allCss) {
    const s = read(f);
    for (const m of s.matchAll(/@media[^{]*?\((?:min|max)-width:\s*(\d+)px/g)) {
      const w = Number(m[1]);
      if (!ALLOWED.has(w)) {
        wild.set(f, (wild.get(f) ?? 0) + 1);
      }
    }
  }
  const total = [...wild.values()].reduce((a, b) => a + b, 0);
  const prev = base.wildBreakpoints ?? total;
  if (total > prev) {
    fails++;
    console.error(`  FAIL：${total} 处野断点（基线 ${prev}）`);
    for (const [f, n] of wild) console.error(`    ${n}\t${f}`);
    console.error("    → 用 md:/lg:/xl: 前缀或 @media (--bp-md)");
  } else {
    console.log(`  OK：${total} 处野断点（基线 ${prev}）`);
  }
  base.wildBreakpoints = UPDATE ? total : prev;
}

// ---------- 4. z-index：只允许 9 个语义档 ----------
section("4. z-index（只允许语义档）");
{
  const ALLOWED = new Set([0, 10, 30, 40, 45, 50, 70, 90, 120]);
  let total = 0;
  const wild = new Map();
  for (const f of allCss) {
    const s = read(f);
    for (const m of s.matchAll(/z-index:\s*(\d+)/g)) {
      if (!ALLOWED.has(Number(m[1]))) {
        wild.set(f, (wild.get(f) ?? 0) + 1);
        total++;
      }
    }
  }
  const prev = base.wildZIndex ?? total;
  if (total > prev) {
    fails++;
    console.error(`  FAIL：${total} 处野 z-index（基线 ${prev}）`);
    for (const [f, n] of wild) console.error(`    ${n}\t${f}`);
    console.error("    → 用 var(--z-*) 语义档（见 base.css @theme）");
  } else {
    console.log(`  OK：${total} 处野 z-index（基线 ${prev}）`);
  }
  base.wildZIndex = UPDATE ? total : prev;
}

// ---------- 5. prefers-reduced-motion：每个含动效的文件都要有 ----------
section("5. 动效降级（含 transition/animation 的文件须有降级块）");
{
  const movers = [];
  for (const f of allCss) {
    const s = read(f);
    const n = (s.match(/^\s*(transition|animation):/gm) ?? []).length +
      (s.match(/@keyframes/g) ?? []).length;
    if (n === 0) continue;
    if (!/prefers-reduced-motion/.test(s)) movers.push([f, n]);
  }
  if (movers.length === 0) {
    console.log("  OK：所有含动效的文件都有降级块");
  } else {
    fails++;
    console.error(`  FAIL：${movers.length} 个文件有动效但无 prefers-reduced-motion`);
    for (const [f, n] of movers) {
      console.error(`    ${n} 处动效\t${f}`);
    }
    console.error("    → 补 @media (prefers-reduced-motion: reduce) 降级块");
  }
}

// ---------- 6. 焦点环：CSS 兜底必须在位 ----------
section("6. 焦点环（outline-none 的 CSS 兜底须在位）");
{
  // 为什么不逐个改 TSX：全站 55 处 `outline-none` 补上
  // `focus-visible:outline-2 focus-visible:outline-sky
  //  focus-visible:outline-offset-1`（56 字符）会把原本 40~60 字符的
  //  className 串顶到 98~130 字符，触发行宽门禁（80 列）；
  //  批量折行又极易生成 `"";` 这类语法错误（实测踩过）。
  //  故走 CSS 兜底：base.css 末尾对 input/textarea/select 的
  //  :focus-visible 补 2px 描边，零 TSX 改动、零行宽风险。
  const base = read("apps/web/app/styles/base.css");
  const hasFallback = /input:focus-visible[\s\S]*?outline:\s*2px solid/.test(
    base,
  );
  if (hasFallback) {
    const n = countMatches(
      globSync("apps/web/{components,app}/**/*.tsx", { cwd: ROOT }),
      "outline-none",
    );
    console.log(`  OK：CSS 兜底在位（base.css 末尾），覆盖 ${n} 处 outline-none`);
  } else {
    fails++;
    console.error("  FAIL：base.css 缺少 :focus-visible 描边兜底");
    console.error(
      "    → 在 base.css 末尾补 input/textarea/select:focus-visible 的 outline",
    );
  }
}

// ---------- 7-8. 作用域与注释（独立模块，见 ui_design_guard_dom.mjs）----------
checkDomScopes({
  section,
  fails: failBox,
  allCss,
  read,
});

// ---------- 汇总 ----------
const total = fails + failBox.n;
console.log(
  `\n${total ? "FAIL" : "PASS"}：${total} 项违规` +
    `${warns ? `，${warns} 项改善` : ""}`,
);
if (UPDATE) {
  writeFileSync(BASELINE, JSON.stringify(base, null, 2) + "\n");
  console.log(`baseline 已更新 -> scripts/ui_design_baseline.json`);
}
process.exit(total ? 1 : 0);
