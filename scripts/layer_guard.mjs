// 分层宪法门禁（FRAME-PLAN-01 §2 / F2，W-48）：三条 grep 规则强制
// 「H 路由层 / S 服务层 / D 领域层」的依赖方向——不靠自觉，靠 CI。
//
// 规则（宪法条款，与 docs/design/frame-plan.md §2 一一对应）：
//   ① H 层禁 SQL：apps/api/src/*_http*/** 与 apps/api/src/*_http.rs 顶层域文件
//      不得引用 sqlx（use 行 / sqlx::query / derive(sqlx::FromRow) 全算）。
//      豁免：repo/（SQL 唯一居住地）与 ALLOW_HTTP_SQL 允许清单。
//   ② S 层禁 HTTP 词汇：apps/api/src/services/** 不得引用 actix_web。
//   ③ D 层双禁：apps/api/src/domain/** 不得引用 actix_web 也不得引用 sqlx。
//
// 存量现实（v4 审查警示入案）：repo/ 现仅 3 文件、auth_http 域 sqlx 调用
// 101 处——分层宪法对存量是**方向不是现状**。因此 ① 按「存量登记 +
// 只许瘦不许胖」棘轮（与 line_limit_guard 同哲学）：
//   · 基线里没有的 H 层文件出现任何 sqlx 引用 → 直接拦（新文件零容忍）；
//   · 基线里的文件计数只许降不许升 → 拦；
//   · 迁移净 SQL（进 repo/ 或 services/）后跑 --update 收缩基线。
// services/ 与 domain/ 是 F2 新建/已有纯层，**没有存量借口**：任何违规
// 直接拦，不进基线。
//
// 用法：node scripts/layer_guard.mjs [--update]（CI 一步；exit 1 = 违规）。
// 红测口径：在 services/ 下临时加 `use actix_web;` 必须被②拦（零容忍）。
import { readFileSync, existsSync, writeFileSync } from "node:fs";
import { globCompat as globSync } from "./lib/glob_compat.mjs";
import path from "node:path";

const ROOT = path.resolve(import.meta.dirname, "..");
const BASELINE = path.resolve(import.meta.dirname, "layer_baseline.json");
const SRC = path.join(ROOT, "apps", "api", "src");

// ① 的 H 层定义：目录名带 _http 后缀的域包 + 顶层散置的 *_http.rs 单文件域。
// repo/ 是 SQL 的家，天然豁免（它就是 R 层本身）。
const HTTP_DIR_RE = /(^|\/)[a-z0-9_]+_http\//i;
const HTTP_TOP_RE = /(^|\/)[a-z0-9_]+_http\.rs$/i;
// 允许清单：住在 http 域命名空间里但不是 handler 的跨层共享件（登记须注明理由）
const ALLOW_HTTP_SQL = [
  // 示例（当前为空）：apps/api/src/settings_http/expected.rs — 无 SQL 纯函数
];

// sqlx 引用计数：use 行 + sqlx:: 路径引用（query/query_as/query_scalar/
// FromRow derive 全覆盖）；按出现次数累计，与摸底 grep 量级一致，只作棘轮比对
const countSqlx = (lines) =>
  lines.reduce(
    (n, l) =>
      n +
      (l.match(/\bsqlx::/g) || []).length +
      (/^\s*use\s+sqlx\b/.test(l) ? 1 : 0),
    0,
  );
const usesActix = (l) =>
  /^\s*use\s+actix_web\b/.test(l) || /\bactix_web::/.test(l);
const usesSqlx = (l) => /^\s*use\s+sqlx\b/.test(l) || /\bsqlx::/.test(l);

// ⚠ strip 行尾 \r（仓库 core.autocrlf=true）：不剥会把 CRLF 文件漏判/误判
const readLines = (rel) =>
  readFileSync(path.join(ROOT, rel), "utf-8")
    .split("\n")
    .map((l) => l.replace(/\r$/, ""))
    .filter((l, i, arr) => !(l === "" && i === arr.length - 1));

const baseline = existsSync(BASELINE)
  ? JSON.parse(readFileSync(BASELINE, "utf-8"))
  : {};

const offenders = [];
const nextBaseline = {};

// ---- ① H 层 sqlx 棘轮 ----------------------------------------------------
const httpFiles = globSync("**/*.rs", { cwd: SRC, nodir: true })
  .map((f) => f.replace(/\\/g, "/"))
  .filter(
    (rel) =>
      !rel.startsWith("repo/") &&
      !ALLOW_HTTP_SQL.includes(`apps/api/src/${rel}`) &&
      (HTTP_DIR_RE.test(rel + "/") || HTTP_TOP_RE.test(rel)),
  );

for (const rel of httpFiles) {
  const full = `apps/api/src/${rel}`;
  const n = countSqlx(readLines(full));
  if (n === 0) continue;
  nextBaseline[full] = n;
  const prev = baseline[full];
  if (prev === undefined) {
    offenders.push({ kind: "new-http-sql", file: full, n });
  } else if (n > prev) {
    offenders.push({ kind: "grew-http-sql", file: full, n, prev });
  }
}

// ---- ② services/ 禁 actix_web（零容忍，无基线） ---------------------------
for (const f of globSync("services/**/*.rs", { cwd: SRC, nodir: true })) {
  const full = `apps/api/src/${f.replace(/\\/g, "/")}`;
  readLines(full).forEach((l, i) => {
    if (usesActix(l))
      offenders.push({
        kind: "svc-actix",
        file: full,
        line: i + 1,
        text: l.trim(),
      });
  });
}

// ---- ③ domain/ 双禁（零容忍，无基线） -------------------------------------
for (const f of globSync("domain/**/*.rs", { cwd: SRC, nodir: true })) {
  const full = `apps/api/src/${f.replace(/\\/g, "/")}`;
  readLines(full).forEach((l, i) => {
    if (usesActix(l))
      offenders.push({
        kind: "dom-actix",
        file: full,
        line: i + 1,
        text: l.trim(),
      });
    if (usesSqlx(l))
      offenders.push({
        kind: "dom-sqlx",
        file: full,
        line: i + 1,
        text: l.trim(),
      });
  });
}

if (process.argv.includes("--update")) {
  writeFileSync(BASELINE, JSON.stringify(nextBaseline, null, 1) + "\n");
  console.log(
    `layer baseline updated: ${Object.keys(nextBaseline).length} 个 ` +
      `H 层文件登记 sqlx 存量计数`,
  );
  process.exit(0);
}

if (offenders.length) {
  console.error(
    "分层宪法门禁违规（FRAME §2：H 禁 SQL 棘轮 / S 禁 actix / D 双禁）：",
  );
  for (const o of offenders) {
    if (o.kind === "new-http-sql")
      console.error(
        `  [① 新文件带SQL] ${o.file} ${o.n} 处 sqlx 引用` +
          `（新 H 层文件零容忍；确属存量遗漏须登记基线并注明理由）`,
      );
    else if (o.kind === "grew-http-sql")
      console.error(
        `  [① 存量变胖] ${o.file} ${o.n} 处 > 基线 ${o.prev} 处` +
          `（只许瘦不许胖；迁移净 SQL 后 --update 收缩基线）`,
      );
    else if (o.kind === "svc-actix")
      console.error(
        `  [② 服务层引用actix] ${o.file}:${o.line} ${o.text}` +
          `（HTTP 词汇不得进用例层）`,
      );
    else if (o.kind === "dom-actix")
      console.error(
        `  [③ 领域层引用actix] ${o.file}:${o.line} ${o.text}`,
      );
    else
      console.error(
        `  [③ 领域层引用sqlx] ${o.file}:${o.line} ${o.text}`,
      );
  }
  process.exit(1);
}
console.log(
  `分层门禁通过（H 层 ${httpFiles.length} 文件纳管，` +
    `sqlx 存量基线 ${Object.keys(baseline).length} 条；` +
    `services/ 与 domain/ 零违规）`,
);
