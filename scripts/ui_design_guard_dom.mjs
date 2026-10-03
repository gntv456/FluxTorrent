/**
 * UI 设计门禁 · DOM/作用域扩展规则（2026-10-03）
 *
 * 从 `ui_design_guard.mjs` 拆出（该文件已 300 行门禁上限）。
 * 两条规则的共同点：**都与「作用域/语义边界」有关**，且都是本轮
 * 真实踩过的坑——语法完全合法、构建正常、静态检查看不出，
 * 只有运行时才暴露。
 *
 *   7. 主题作用域：`[a="b"][c="d"]` 相邻双属性永不匹配
 *   8. CSS 注释：注释文本里含 `/*` 会吃掉后续所有内容
 *
 * 导出 `checkDomScopes(ctx)`，由主脚本调用并累加 fails。
 * 单独 `node scripts/ui_design_guard_dom.mjs` 也能跑（自检模式）。
 */
import { readFileSync } from "node:fs";
import { globCompat as globSync } from "./lib/glob_compat.mjs";
import path from "node:path";

const ROOT = path.resolve(import.meta.dirname, "..");
const read = (rel) => readFileSync(path.join(ROOT, rel), "utf8");

/**
 * @param {{section:(t:string)=>void, fails:{n:number},
 *          allCss:string[], read:(rel:string)=>string}} ctx
 */
export function checkDomScopes(ctx) {
  const { section, allCss, read: rd } = ctx;

  // ---------- 7. 主题作用域：相邻双属性选择器 ----------
  section("7. 主题作用域（双属性选择器）");
  {
    // 2026-10-03 踩过的坑：arcade-sweet.css 里有 32 条夜间覆盖写成
    //   [data-arcade="sweet"][data-theme="baozi-night"] .x { … }
    // 双属性选择器要求**同一元素**同时有两个属性。但 data-arcade 挂在
    // games/layout 的内层 div 上，data-theme 挂在 <html> 上 —— 两者
    // 永不相交，**整块夜间覆盖从未生效过**（娱乐屋夜间一直发白的真因）。
    //
    // 正确写法是后代选择器：[data-theme="baozi-night"] [data-arcade="sweet"] .x
    // 这类 bug 静态看不出、运行时才发现（无头浏览器量 computed style），
    // 故做成门禁：任何 `[a="b"][c="d"]` 形式的主题组合一律 FAIL。
    const bad = [];
    for (const f of allCss) {
      const s = rd(f);
      const lines = s.split("\n");
      let inBlockComment = false;
      lines.forEach((ln, i) => {
        // 跳过注释内容（块注释与行注释），只查真正的规则行
        const t = ln.trim();
        if (inBlockComment) {
          if (t.includes("*/")) inBlockComment = false;
          return;
        }
        if (t.startsWith("/*") && !t.includes("*/")) {
          inBlockComment = true;
          return;
        }
        if (t.startsWith("/*") || t.startsWith("*")) return;
        // 匹配形如 [a="b"][c="d"] 的**相邻**双属性（中间无空格/逗号）。
        // 后代选择器 `[a="b"] [c="d"]` 中间有空格，是**正确**写法，不算。
        if (/\[[a-z-]+=["'][^\]]*["']\]\[[a-z-]+=/.test(ln)) {
          bad.push(`    ${f}:${i + 1}  ${t.slice(0, 58)}`);
        }
      });
    }
    if (bad.length === 0) {
      console.log("  OK：无相邻双属性主题选择器");
    } else {
      ctx.fails.n += bad.length;
      console.error(
        `  FAIL：${bad.length} 处相邻双属性选择器（除非两属性真在同一元素上）`,
      );
      for (const x of bad.slice(0, 10)) console.error(x);
      console.error(
        "    → 若两属性不在同一元素（如 data-theme 在 <html>、data-arcade",
        "      在内层 div），改成后代选择器 [data-theme=…] [data-arcade=…] .x",
      );
    }
  }

  // ---------- 8. CSS 注释配平 ----------
  section("8. CSS 注释（配平 + 注释内无 /*）");
  {
    // 2026-10-03 踩过三次的坑：注释文本里写路径 `/games/*` 或类名
    // `.gc-*/.arc-*` 时，其中的 `/*` 会被 CSS 解析成**新的注释开启符**，
    // 吃掉后续所有内容直到遇见下一个 `*/` —— 表现为构建期
    // `Error: Unexpected '/'`，TS 与 ESLint 都查不出（不是它们的领域）。
    // 三处分别是 arcade-gacha.css / arcade-sweet.css / globals.css。
    const bad = [];
    for (const f of allCss) {
      const s = rd(f);
      const stack = [];
      const re = /\/\*|\*\//g;
      let m;
      while ((m = re.exec(s))) {
        const ln = s.slice(0, m.index).split("\n").length;
        if (m[0] === "/*") {
          stack.push(ln);
        } else if (stack.length) {
          stack.pop();
        } else {
          bad.push(`    ${f}:${ln}  多余的 */`);
        }
      }
      for (const ln of stack) {
        bad.push(`    ${f}:${ln}  /* 未闭合（疑似注释文本里含 /*）`);
      }
    }
    if (bad.length === 0) {
      console.log("  OK：所有 CSS 注释配平");
    } else {
      ctx.fails.n += bad.length;
      console.error(`  FAIL：${bad.length} 处 CSS 注释未闭合/多余`);
      for (const x of bad.slice(0, 10)) console.error(x);
      console.error(
        "    → 注释里不要写 /games/* 或 .gc-* 这类含 /* 的串，改用文字描述",
      );
    }
  }
}

// 自检模式：单独跑本文件
if (process.argv[1] && import.meta.url.endsWith(
  path.basename(process.argv[1]).replace(/\\/g, "/").split("/").pop(),
)) {
  const allCss = [
    ...globSync("apps/web/app/styles/*.css", { cwd: ROOT }),
    "apps/web/app/globals.css",
  ];
  const fails = { n: 0 };
  checkDomScopes({
    section: (t) => console.log(`\n== ${t} ==`),
    fails,
    allCss,
    read,
  });
  console.log(`\n${fails.n ? "FAIL" : "PASS"}：${fails.n} 项违规`);
  process.exit(fails.n ? 1 : 0);
}
