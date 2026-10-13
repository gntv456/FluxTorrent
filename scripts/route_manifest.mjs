// 路由清单生成器（W-12 / FRAME-PLAN-01 §3 步①）：从 apps/api Rust 源码静态
// 提取全部 actix 路由宏，产出 docs/api/route-manifest.json 机器可读清单——
// 不依赖 utoipa，先拿到「路由真相」，供门禁与契约工程后续步骤对账。
//
// 提取口径（与对标报告 §13.1 的 grep 基数一致，当前 745 条）：
//   grep -rE '#\[(actix_web::)?(get|post|put|patch|delete|route)\(' \
//     apps/api/src --include=*.rs | wc -l
//   1) 方法宏六种：#[get|post|put|patch|delete|route("/path")]，含
//      #[actix_web::get(...)] 全限定形态（历史坑：email_policy.rs 用过，
//      按裸宏名抓会漏 1 条）；#[route] 额外认 method = "GET" 尾参
//      （本仓当前 0 处，留前向兼容）。
//   2) handler 关联：宏与函数之间允许 doc 注释/块注释/其他属性行，取其后
//      第一条 (pub …)?(async )?fn 名；关联不上记 null（不阻断提取）。
//   3) path 记宏原始字面量 = scope 相对路径：/api/v1 等挂载前缀由 mount 侧
//      拼接，mount 链复杂，清单不还原（见输出 JSON 的 path_scope 注记）。
//   4) 清单条目只存 {method, path, handler, file}，不存行号——文件上方插几
//      行注释不该让防漂移门禁挂掉；行号只用于 (file, line) 排序。
//
// 输出确定性：无时间戳（generated_at 会让每次生成产生无意义 diff，破坏
// 「同一提交在任何机器上字节级一致」），排序稳定，git diff 才有意义。
//
// 用法：
//   node scripts/route_manifest.mjs [--write|--update]  # 生成/刷新清单
//   node scripts/route_manifest.mjs --check             # 对账，漂移 exit 1
// 门禁：scripts/route_manifest_guard.mjs 复用本模块的 compareWithStored。
import { readFileSync, writeFileSync, existsSync, realpathSync } from "node:fs";
import { globCompat as globSync } from "./lib/glob_compat.mjs";
import path from "node:path";
import { fileURLToPath } from "node:url";

const ROOT = path.resolve(import.meta.dirname, "..");
const OUT = path.join(ROOT, "docs", "api", "route-manifest.json");
export const OUT_REL = "docs/api/route-manifest.json";
export const HINT =
  `路由变更须同步 ${OUT_REL}，` +
  `运行 node scripts/route_manifest.mjs --write`;
// 行宏：六种方法名 + actix_web:: 全限定；路径须双引号字面量；
// #[route] 的 method = "GET" 尾参可选（捕获组 3）
const RE_ROUTE = new RegExp(
  '#\\[(?:actix_web::)?(get|post|put|patch|delete|route)' +
  '\\("([^"]+)"(?:\\s*,\\s*method\\s*=\\s*"([A-Z]+)")?\\)\\]',
);
// handler 首行：pub/pub(crate) 可选、async 可选（actix 也允许同步 fn，宽松）
const RE_FN =
  /^\s*(?:pub(?:\([^)]*\))?\s+)?(?:async\s+)?fn\s+([A-Za-z0-9_]+)/;

// 宏行向下找 handler：跳过空行/注释/属性行，遇 fn 取名，遇其他代码即放弃
// （宏必须贴 fn，中间只可能是注释与属性——见头注口径 2）。
function findHandler(lines, from) {
  for (let j = from + 1; j < lines.length && j <= from + 40; j++) {
    const t = lines[j].trim();
    if (t === "" || t.startsWith("//") || t.startsWith("#[") ||
      t.startsWith("*")) continue;
    const fm = RE_FN.exec(lines[j]);
    return fm ? fm[1] : null;
  }
  return null;
}

// 提取全部路由：[{method, path, handler, file, line}]，按 (file, line) 排序。
// 不去重——与 grep 基数同口径（逐宏计数）；同文件同路径重复挂载是真实
// 信息，保留给人审。
// 注释里的宏不算路由（grep 不知道注释，本脚本知道——被注释掉的路由是
// 死代码，留在清单里会永久虚计）：匹配点之前同一直行内出现过 // 或 /*
// （行注释/块注释开启），或行首是块注释续行 *，即跳过。
export function extract() {
  const files = globSync("apps/api/src/**/*.rs", { cwd: ROOT, nodir: true })
    .map((f) => f.replace(/\\/g, "/")).sort();
  const routes = [];
  let commented = 0;
  for (const rel of files) {
    const lines = readFileSync(path.join(ROOT, rel), "utf-8")
      .split("\n").map((l) => l.replace(/\r$/, ""));
    for (let i = 0; i < lines.length; i++) {
      const m = RE_ROUTE.exec(lines[i]);
      if (!m) continue;
      const before = lines[i].slice(0, m.index);
      if (before.includes("//") || before.includes("/*") ||
        /^\s*(\/\*|\*)/.test(lines[i])) {
        commented++;
        continue;
      }
      routes.push({
        // #[route] 的方法在 method 尾参；六种方法宏直接用宏名
        method: m[1] === "route" ? (m[3] ?? "ROUTE?") : m[1].toUpperCase(),
        path: m[2],
        handler: findHandler(lines, i),
        file: rel,
        line: i + 1,
      });
    }
  }
  routes.sort((a, b) =>
    a.file < b.file ? -1 : a.file > b.file ? 1 : a.line - b.line);
  return { routes, fileCount: files.length, commented };
}

// 序列化清单（字节确定）：条目剔除 line（见头注口径 4）。
export function buildDump() {
  const { routes, fileCount, commented } = extract();
  const manifest = {
    note: "actix 路由宏静态提取：get/post/put/patch/delete/route 六种" +
      "方法宏，含 #[actix_web::get] 全限定形态；handler 关联自宏后首条" +
      " fn；注释内的宏不计（被注释掉的路由是死代码）",
    path_scope: "path=宏原始字面量（scope 相对路径）；/api/v1 等挂载前缀" +
      "由 mount 侧拼接，清单不还原 mount 链",
    grep_baseline_note: "口径对齐 grep -rE '#\\[(actix_web::)?(get|post|" +
      "put|patch|delete|route)\\(' apps/api/src --include=*.rs；grep 会" +
      "数到注释里的宏，total 若比 grep 少，差额即被注释掉的死宏",
    total: routes.length,
    routes: routes.map(({ method, path, handler, file }) => ({
      method, path, handler, file,
    })),
  };
  return {
    dump: JSON.stringify(manifest, null, 1) + "\n",
    routes,
    fileCount,
    commented,
  };
}

// 与库中清单对账：按 (method, path, file) 三元组对集合，handler 改名也算
// 漂移；行号与顺序不参与（生成已稳定排序，重排自身也是漂移，无需比较）。
export function compareWithStored() {
  if (!existsSync(OUT)) return { missing: true };
  const storedRoutes = JSON.parse(readFileSync(OUT, "utf-8")).routes ?? [];
  const { routes } = extract();
  const key = (r) => `${r.method} ${r.path} @ ${r.file}`;
  const cur = new Map(routes.map((r) => [key(r), r.handler]));
  const old = new Map(storedRoutes.map((r) => [key(r), r.handler]));
  const added = [...cur.keys()].filter((k) => !old.has(k));
  const removed = [...old.keys()].filter((k) => !cur.has(k));
  const changed = [...cur.keys()].filter((k) =>
    old.has(k) && old.get(k) !== cur.get(k));
  return {
    ok: added.length === 0 && removed.length === 0 && changed.length === 0,
    added, removed, changed,
    storedTotal: storedRoutes.length,
    total: routes.length,
  };
}

function main() {
  const argv = process.argv.slice(2);
  if (argv.includes("--check")) {
    const r = compareWithStored();
    if (r.missing) {
      console.error(`ROUTE_MANIFEST_FAIL ${OUT_REL} 不存在；${HINT}`);
      process.exit(1);
    }
    if (r.ok) {
      console.log(`ROUTE_MANIFEST_OK ${r.total} 条路由与清单一致`);
      process.exit(0);
    }
    const MAX = 20; // 每类最多列 20 条，全量几百条会淹没 CI 日志
    console.error(
      `ROUTE_MANIFEST_FAIL 清单过期：库中 ${r.storedTotal} 条，源码 ` +
      `${r.total} 条（新增 ${r.added.length} / 移除 ${r.removed.length}` +
      ` / handler 改名 ${r.changed.length}）`);
    const groups = [
      ["+", r.added],
      ["-", r.removed],
      ["~", r.changed],
    ];
    for (const [tag, list] of groups)
      list.slice(0, MAX).forEach((k) => console.error(`  ${tag} ${k}`));
    console.error(`修复：${HINT}`);
    process.exit(1);
  }
  const { dump, routes, fileCount, commented } = buildDump();
  writeFileSync(OUT, dump);
  const noHandler = routes.filter((r) => !r.handler).length;
  console.log(
    `${OUT_REL} 已生成：${routes.length} 条路由 / ${fileCount} 个文件` +
    `${commented ? `（另有 ${commented} 处注释内宏未计入）` : ""}` +
    `${noHandler ? `（${noHandler} 条未关联到 handler 名）` : ""}`);
}

// 直接运行才执行 CLI；被 route_manifest_guard.mjs import 时只暴露函数
const isMain = (() => {
  if (!process.argv[1]) return false;
  try {
    return realpathSync(process.argv[1]) ===
      realpathSync(fileURLToPath(import.meta.url));
  } catch {
    return false;
  }
})();
if (isMain) main();
