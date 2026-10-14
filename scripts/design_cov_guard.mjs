// 设计系统覆盖率门禁（DS-PLAN-01 W-07 / §5：意图三/四的载体）。
//
// 四个数字（「成熟」不是形容词，是可计算、进 CI、只许向好的指标）：
//   1. routeLoading  路由加载态覆盖 = 有 loading.tsx 或显式骨架的数据路由 /
//                      数据路由总数。数据路由判定（启发式，可复算）：
//                      page.tsx **代码**（已剥注释）含 api.* 调用 /
//                      searchParams / force-dynamic 导出 / Suspense /
//                      StatePanel / Skeleton 之一，**且含 JSX**；
//                      覆盖 = 同目录有 loading.tsx 或页面内显式骨架。
//                      2026-10-14 口径修正（缺陷修复，见 §5.1 注）：
//                      原实现整文件正则，纯转发壳页的**文档注释**里出现
//                      "force-dynamic" 字样即被误判；且无 JSX 的壳页
//                      没有可骨架的 UI。两类误报已剔除（75→72）。
//   2. primitiveAdoption 原语采用率 = import @/components/ui 的 tsx /
//                      全部 tsx（components+app）。
//   3. nonExemptHex  非豁免硬编码 hex（games 皮肤资产豁免：components/game、
//                      (main)/games、(main)/gacha；同 DS §6 口径）。
//   4. a11y 骨架     <main> 标签数（每路由组布局应有且仅有一个 main 锚点）
//                      + SkipLink 挂载（硬性：缺失即 FAIL）。
//
// 基线模式照抄 line_limit_baseline.json：存量登记、只许向好——
//   routeLoading/primitiveAdoption/mainTags 只许升；nonExemptHex 只许降。
// 用法：node scripts/design_cov_guard.mjs [--update]
//   exit 0 = 通过（可能打印缺口清单引导清偿）; exit 1 = 指标恶化。
import { readFileSync, writeFileSync, existsSync } from "node:fs";
import { globCompat as globSync } from "./lib/glob_compat.mjs";
import path from "node:path";

const ROOT = path.resolve(import.meta.dirname, "..");
const WEB = path.join(ROOT, "apps", "web");
const BASELINE = path.resolve(import.meta.dirname, "design_cov_baseline.json");
const UPDATE = process.argv.includes("--update");

const read = (rel) => readFileSync(path.join(ROOT, rel), "utf8");

// ---------- 文件收集 ----------
const tsxFiles = globSync("apps/web/{components,app}/**/*.tsx", {
  cwd: ROOT,
})
  .map((f) => f.replace(/\\/g, "/"))
  .filter((f) => !f.includes("/.next/"));
const pageFiles = tsxFiles.filter((f) => f.endsWith("/page.tsx"));
// games 豁免（DS §6：SVG 皮肤是内容不是 chrome）
const EXEMPT_HEX = [
  "apps/web/components/game/",
  "apps/web/app/(main)/games/",
  "apps/web/app/(main)/gacha/",
];

// ---------- 指标 1：路由加载态覆盖 ----------
// 判据缺陷修复（2026-10-14）——两条都是「正则读到了不该读的东西」：
//   ① 剥注释：`/* */` 与整行 `//` 先移除再匹配。实证误报：
//      app/(main)/me、app/(main)/forums/new、
//      app/(main)/users/[id]/torrentlist 三页的文档注释里写了
//      「无需 force-dynamic」→ 旧正则在散文里命中，三页被当成数据路由。
//   ② 要求 JSX：loading.tsx 的语义是「先出骨架再填数据」，纯转发壳页
//      （redirect 无 UI）没有可骨架的对象；`me` 页注释亦自述
//      「加载态由路由组 (main) 框架壳承担」。无 JSX 即不属数据路由。
//   ③ `force-dynamic` 由裸词收紧为真实导出语句（防注释/字符串误命中）。
// 未收编（留待口径决策，见 DS §5.1 待议）：`"use client"` 页里的
// `await api.` / `api.get(`——7 条客户端表单页的 api 调用发生在**提交期**
// 而非渲染期，其「加载态」是按钮 busy 态而非路由骨架。收编即 100%。
const stripComments = (s) =>
  s.replace(/\/\*[\s\S]*?\*\//g, "").replace(/^\s*\/\/.*$/gm, "");
const srcOf = (rel) => stripComments(read(rel));
const DATA_HINTS = new RegExp(
  [
    "await api\\.",
    "api\\.(get|post|put|delete)",
    "searchParams",
    "export const dynamic\\s*=\\s*[\"']force-dynamic[\"']",
    "<Suspense",
    "StatePanel",
    "Skeleton",
  ].join("|"),
);
const hasJsx = (rel) => /<[A-Za-z][A-Za-z0-9.]*[\s/>]/.test(srcOf(rel));

/**
 * 非数据路由登记（2026-10-14，D1 建议 B 落地，DS §5.1.1 待议项收编）。
 *
 * `loading.tsx` 是 **RSC 路由级**加载态，只对「服务端渲染即取数」的路由有意义。
 * 以下页面不属数据路由，**不计入分母**；每条必须带理由（同 `EXEMPT_HEX`
 * 的登记纪律，新增须先人工复核）。收编后数据路由 72→65、覆盖 65/65=100%。
 */
const NOT_DATA_ROUTES = {
  // ── 客户端表单页：api 调用发生在**提交期**（onClick/onSubmit），
  //    非渲染期取数；其加载态是按钮 busy（SubmitButton/ActionFeedback）
  "app/setup/page.tsx":
    "安装向导（客户端表单，提交期 POST；无渲染期取数）",
  "app/(auth)/self-unban/page.tsx": "客户端表单（提交期 POST）",
  "app/(auth)/reset/page.tsx":
    "客户端表单（提交期 POST；已有内联 <Suspense> 骨架）",
  "app/(auth)/forgot/page.tsx": "客户端表单（提交期 POST）",
  "app/(auth)/apply/page.tsx": "客户端表单（提交期 POST）",
  // ── 客户端渲染期取数页：取数发生在 hydration 之后（useEffect），
  //    loading.tsx 的触发时机对它无效（骨架应做在页内）
  "app/(auth)/register/page.tsx": "客户端渲染期取验证码（hydration 后）",
  "app/(auth)/email-verify/page.tsx": "客户端渲染期取验证状态（hydration 后）",
};

const dataRoutes = pageFiles
  .filter(
    (f) =>
      DATA_HINTS.test(srcOf(f)) &&
      hasJsx(f) &&
      !NOT_DATA_ROUTES[f.replace(/\\/g, "/").replace(/^apps\/web\//, "")],
  )
  .map((f) => f.replace(/\\/g, "/"));
const coveredRoutes = dataRoutes.filter((f) => {
  const dir = path.posix.dirname(f);
  const hasLoading = tsxFiles.some(
    (g) => path.posix.dirname(g) === dir && g.endsWith("/loading.tsx"),
  );
  const inlineSkeleton = /Skeleton|StatePanel/.test(srcOf(f));
  return hasLoading || inlineSkeleton;
});
const routeLoading = dataRoutes.length
  ? Math.round((coveredRoutes.length / dataRoutes.length) * 100)
  : 100;

// ---------- 指标 2：原语采用率 ----------
const adopting = tsxFiles.filter((f) =>
  /from ["']@\/components\/ui/.test(read(f)),
).length;
const primitiveAdoption = tsxFiles.length
  ? Math.round((adopting / tsxFiles.length) * 1000) / 10
  : 0;

// ---------- 指标 3：非豁免 hex ----------
// 行级豁免约定：本行或上一行含 `design-cov-exempt` 注释不计（平台强制
// 字面量，如 viewport themeColor / manifest background_color——meta 与
// manifest 是安装时读取的静态数据，吃不到 CSS 变量）。豁免必须带原因
// 注释，禁止用来洗可令牌化的色值。
const EXEMPT_MARK = "design-cov-exempt";
const hexFiles = globSync("apps/web/{app,components,lib,i18n}/**/*.{ts,tsx}", {
  cwd: ROOT,
}).map((f) => f.replace(/\\/g, "/"));
let nonExemptHex = 0;
const hexPerFile = {};
for (const f of hexFiles) {
  if (EXEMPT_HEX.some((p) => f.startsWith(p))) continue;
  const lines = read(f).split("\n");
  let n = 0;
  lines.forEach((l, i) => {
    const prev = i > 0 ? lines[i - 1] : "";
    if (l.includes(EXEMPT_MARK) || prev.includes(EXEMPT_MARK)) return;
    n += (l.match(/#[0-9a-fA-F]{6}\b/g) ?? []).length;
  });
  if (n > 0) {
    hexPerFile[f] = n;
    nonExemptHex += n;
  }
}

// ---------- 指标 4：a11y 骨架 ----------
const mainTags = tsxFiles.filter((f) => /<main[\s>]/.test(read(f))).length;
const skipLinkMounted = tsxFiles.some(
  (f) => /SkipLink/.test(read(f)) && f.endsWith("app/layout.tsx"),
);

// ---------- 基线比对（只许向好） ----------
const base = existsSync(BASELINE)
  ? JSON.parse(readFileSync(BASELINE, "utf8"))
  : null;
const cur = {
  routeLoading,
  primitiveAdoption,
  nonExemptHex,
  mainTags,
  skipLinkMounted,
};

const fails = [];
const notes = [];
if (!base) {
  notes.push("无基线：首跑请审阅下表后用 --update 落盘");
} else {
  if (cur.routeLoading < base.routeLoading)
    fails.push(
      `路由加载态覆盖 ${cur.routeLoading}% < 基线 ${base.routeLoading}%`,
    );
  if (cur.primitiveAdoption < base.primitiveAdoption)
    fails.push(
      `原语采用率 ${cur.primitiveAdoption}% < 基线 ${base.primitiveAdoption}%`,
    );
  if (cur.nonExemptHex > base.nonExemptHex)
    fails.push(
      `非豁免 hex ${cur.nonExemptHex} > 基线 ${base.nonExemptHex}（只许降）`,
    );
  if (cur.mainTags < base.mainTags)
    fails.push(`<main> ${cur.mainTags} < 基线 ${base.mainTags}`);
  if (!cur.skipLinkMounted)
    fails.push("SkipLink 未挂载到根布局（app/layout.tsx）");
}

// ---------- 输出 ----------
const pct = (n, d) => `${n}/${d}`;
console.log("== 设计系统覆盖率（DS-PLAN-01 §5.1）==");
console.log(
  `  路由加载态覆盖   ${cur.routeLoading}%  ` +
    `（${pct(coveredRoutes.length, dataRoutes.length)} 数据路由）` +
    (base ? `  基线 ${base.routeLoading}%` : ""),
);
console.log(
  `  原语采用率       ${cur.primitiveAdoption}%  ` +
    `（${pct(adopting, tsxFiles.length)} tsx）` +
    (base ? `  基线 ${base.primitiveAdoption}%` : ""),
);
console.log(
  `  非豁免 hex       ${cur.nonExemptHex}  ` +
    `（games 豁免后；只许降）` +
    (base ? `  基线 ${base.nonExemptHex}` : ""),
);
console.log(
  `  a11y 骨架        <main>×${cur.mainTags}  SkipLink=${
    cur.skipLinkMounted ? "已挂载" : "缺失"
  }` + (base ? `  基线 <main>×${base.mainTags}` : ""),
);
// 缺口清单（ informational：引导清偿，不拦合并 ）
const missingLoading = dataRoutes.filter((f) => !coveredRoutes.includes(f));
if (missingLoading.length) {
  console.log(
    `\n  缺口·无加载态数据路由 ${missingLoading.length} 条（前 10）：`,
  );
  for (const f of missingLoading.slice(0, 10))
    console.log(`    ${f.replace("apps/web/", "")}`);
}
const hexTop = Object.entries(hexPerFile)
  .sort((a, b) => b[1] - a[1])
  .slice(0, 10);
if (hexTop.length) {
  console.log("\n  缺口·hex Top10（清偿引导）：");
  for (const [f, n] of hexTop)
    console.log(`    ${n}\t${f.replace("apps/web/", "")}`);
}
for (const n of notes) console.log(`\n  提示：${n}`);

if (UPDATE) {
  writeFileSync(BASELINE, JSON.stringify(cur, null, 2) + "\n");
  console.log(`\nbaseline 已更新 -> scripts/design_cov_baseline.json`);
  process.exit(0);
}
if (fails.length) {
  console.error("\nFAIL：覆盖率指标恶化（只许向好）：");
  for (const f of fails) console.error(`  ${f}`);
  process.exit(1);
}
console.log("\nPASS：四指标无恶化");
