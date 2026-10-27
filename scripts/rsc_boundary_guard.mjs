// RSC 边界门禁（rsc_boundary_guard.mjs）
//
// 背景（2026-10-27 现网翻车）：帖子页 `/forums/topic/<id>` 整页落错误边界，
// 控制台 `[route-error]`。根因是 forum-torrent-card.tsx 带 `"use client"`，
// 却把纯函数 `torrentLine` 一并导出，被无指令的 forum-markdown.tsx（服务端
// 渲染）导入并在服务端调用 ⇒ Next 抛「Attempted to call torrentLine() from
// the server but torrentLine is on the client」。
// 同族第二处：admin/[...slug]/page.tsx 读 client 模块 admin-shared.ts 的常量
// LEGACY_TOOL ⇒ 服务端拿到 undefined，旧书签别名静默失效（/admin/reset 落
// ?tool=reset 而非 resetpass）。
//
// 为什么必须由静态门拦：`tsc --noEmit` 与 `next build` 全绿（CI 里就有一步
// next build），类型上「client 模块的导出」与「普通模块的导出」毫无区别；
// 只有运行期请求才炸。而炸点在 RSC 载荷 stringify 阶段 ⇒ HTML 里楼层正文
// 仍在，肉眼 curl 判不出来，只有浏览器可见文本 + console 能定性。
//
// 口径（对齐 Next 的模块图语义，不做全图分析）：
//   1) 服务端可达集：从 app/** 里**无 "use client"** 的路由模块
//      （page/layout/template/route/error/not-found/loading/default）出发，
//      沿静态 import 闭包扩张；一旦走进带指令的 client 模块就停——它下面
//      整条链都是浏览器侧，不算服务端可达。
//   2) 违规：可达集内的模块从 client 模块导入「值绑定」并在服务端用它：
//      · 小写/下划线开头的绑定（函数、hook、常量）⇒ 直接拦（服务端拿到的是
//        client reference，调用即抛、取下标得 undefined）；
//      · PascalCase 绑定（组件）本属合法（当元素渲染/当 prop 传），但把它
//        当函数调用（`Name(`）或取属性（`Name.x`）同样违规 ⇒ 拦。
//   3) `import type` 与 `type` 说明符在编译期被抹平，不算。
//   4) tests/ 与 e2e/ 用 jsdom 直跑源码，无 RSC 边界，排除。
//
// 存量：2026-10-27 建门时为 0 处（forum/admin 两处已随本批修掉）——故不设
// 基线，新违规直接红灯。
//
// 用法：node scripts/rsc_boundary_guard.mjs        # exit 1 = 有违规
// 红测口径（三条分支各有正控，见 _tmp 控制脚本）：
//   ① 值·函数：forum-markdown.tsx 把 torrentLine 改回从 client 的
//      forum-torrent-card 导入 ⇒ ① 拦；
//   ② 值·常量：my/page.tsx 把 USERCP_NAV 改回从 client 的 usercp 导入 ⇒ ② 拦；
//   ③ 组件被调用：把 <UsercpPanel …/> 改成 UsercpPanel({…}) ⇒ ③ 拦。
import {
  readFileSync,
  readdirSync,
  statSync,
  existsSync,
} from "node:fs";
import path from "node:path";

const ROOT = path.resolve(import.meta.dirname, "..");
const WEB = path.join(ROOT, "apps", "web");
const SKIP_DIRS = new Set([
  "node_modules",
  ".next",
  ".turbo",
  "coverage",
  "test-results",
  "playwright-report",
  "tests",
  "e2e",
  "public",
]);
const ROUTE_NAMES = new RegExp(
  "^(page|layout|template|route|error|global-error|not-found|" +
    "loading|default)\\.(ts|tsx)$",
);

function walk(dir, out) {
  for (const entry of readdirSync(dir)) {
    if (SKIP_DIRS.has(entry)) continue;
    const p = path.join(dir, entry);
    const st = statSync(p);
    if (st.isDirectory()) walk(p, out);
    else if (/\.tsx?$/.test(entry)) out.push(p);
  }
  return out;
}

const FILES = walk(WEB, []);
const TEXT = new Map(FILES.map((f) => [f, readFileSync(f, "utf8")]));

/** 剥注释后再扫 import（否则注释里的一句话能造出假违规，也能洗白真违规） */
function stripComments(s) {
  return s
    .replace(/\/\*[\s\S]*?\*\//g, "")
    .replace(/^\s*\/\/[^\n]*$/gm, "");
}

/** "use client"/"use server" 指令：必须在任何真实代码之前（注释可在前） */
const DIRECTIVE_RE = new RegExp(
  "^\\s*(?:/\\*[\\s\\S]*?\\*/|//[^\\n]*\\n|\\s)*[\"']use (client|server)[\"']",
);
function hasClientDirective(s) {
  return DIRECTIVE_RE.test(s.slice(0, 2000));
}

const CLIENT = new Set(FILES.filter((f) => hasClientDirective(TEXT.get(f))));

/** 解析模块说明符 → 仓内文件（@/ 走 apps/web 根；未解析的外部包/类型包返回 null） */
function resolveSpec(mod, fromFile) {
  let base = null;
  if (mod.startsWith("@/")) base = path.join(WEB, mod.slice(2));
  else if (mod.startsWith(".")) {
    base = path.resolve(path.dirname(fromFile), mod);
  } else return null;
  for (const cand of [
    base + ".tsx",
    base + ".ts",
    base + ".js",
    path.join(base, "index.tsx"),
    path.join(base, "index.ts"),
  ]) {
    if (TEXT.has(cand)) return cand;
  }
  return null;
}

// 逐文件解析（保留文件路径给 target 解析用）
function importsOf(file) {
  const code = stripComments(TEXT.get(file));
  const out = [];
  const re = new RegExp(
    "^import\\s+(\\{[^}]*\\}|(?:type\\s+)?[A-Za-z_$][\\w$]*|" +
      "\\*\\s+as\\s+[A-Za-z_$][\\w$]*)?\\s*from\\s*[\"']([^\"']+)[\"']",
    "gm",
  );
  for (const m of code.matchAll(re)) {
    const spec = m[1] ?? "";
    const target = resolveSpec(m[2], file);
    if (!target) continue;
    const bindings = [];
    if (spec.startsWith("{")) {
      for (const raw of spec.slice(1, -1).split(",")) {
        const s = raw.trim();
        if (!s) continue;
        if (s.startsWith("type ")) continue;
        const asMatch = s.match(/^([A-Za-z_$][\w$]*)(?:\s+as\s+(.+))?$/);
        if (!asMatch) continue;
        bindings.push({
          imported: asMatch[1],
          local: (asMatch[2] ?? asMatch[1]).trim(),
        });
      }
    } else if (spec.startsWith("*")) {
      // `import * as ns from "client"`：服务端没有合法用法（拿不到真值），整条拦
      bindings.push({ imported: "*", local: spec.replace(/^\*\s+as\s+/, "") });
    } else if (spec && !spec.startsWith("type")) {
      bindings.push({ imported: "default", local: spec });
    }
    out.push({ target, bindings, raw: m[0] });
  }
  return out;
}

// ---- 1) 服务端可达集 ----
const APP_DIR = path.join(WEB, "app") + path.sep;
const roots = FILES.filter(
  (f) =>
    f.startsWith(APP_DIR) &&
    ROUTE_NAMES.test(path.basename(f)) &&
    !CLIENT.has(f),
);
const serverReachable = new Set();
const queue = [...roots];
while (queue.length) {
  const f = queue.pop();
  if (serverReachable.has(f)) continue;
  serverReachable.add(f);
  for (const imp of importsOf(f)) {
    if (CLIENT.has(imp.target)) continue; // 跨进 client 就停
    queue.push(imp.target);
  }
}

// ---- 2) 违规判定 ----
const violations = [];
for (const file of serverReachable) {
  const code = stripComments(TEXT.get(file));
  for (const imp of importsOf(file)) {
    if (!CLIENT.has(imp.target)) continue;
    for (const { local, imported } of imp.bindings) {
      if (!local || local === "type") continue;
      if (imported === "*") {
        violations.push({
          file,
          target: imp.target,
          local,
          why: "命名空间导入：服务端取不到 client 模块的真值",
        });
        continue;
      }
      // PascalCase 才是组件；UPPER_SNAKE（常量）与 camelCase（函数/hook）都按值算
      const isComponent = /^[A-Z][A-Za-z0-9]*$/.test(local);
      if (!isComponent) {
        violations.push({
          file,
          target: imp.target,
          local,
          why: "非组件值（函数/hook/常量）：服务端拿到的是 client reference",
        });
        continue;
      }
      // 组件：只拦「被当函数调用 / 被取属性」，渲染与传 prop 合法
      const callRe = new RegExp(
        `(?<![\\w$.])${local.replace(/\$/g, "\\$")}\\s*(?:\\(|\\.[A-Za-z_$])`,
      );
      if (callRe.test(code)) {
        violations.push({
          file,
          target: imp.target,
          local,
          why: "组件被当函数调用/取属性，不是渲染",
        });
      }
    }
  }
}

const rel = (p) => path.relative(ROOT, p).replaceAll("\\", "/");
if (!existsSync(path.join(WEB, "app"))) {
  console.error(`RSC 边界门禁：找不到 ${rel(path.join(WEB, "app"))}（口径失效，拒判）`);
  process.exit(2);
}
if (violations.length) {
  console.error(
    `RSC 边界违规 ${violations.length} 处（服务端可达模块用了 client 模块的值）：\n`,
  );
  for (const v of violations) {
    console.error(
      `  ${rel(v.file)}  ←  ${v.local}  (client: ${rel(v.target)})`,
    );
    console.error(`     ${v.why}`);
    console.error(
      "     修法：把该导出挪进无指令的中性模块（lib/*.ts），两边各自导入。",
    );
  }
  process.exit(1);
}
console.log(
  `RSC 边界门禁：0 违规（服务端可达 ${serverReachable.size} 个模块、` +
    `client 模块 ${CLIENT.size} 个、路由根 ${roots.length} 个）`,
);
