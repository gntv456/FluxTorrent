// 文件行数门禁（防膨胀）：新文件不许超限，存量超限文件不许再变大。
// 规则（对齐团队「按域拆分」的既有实践，如 http.rs 六段外移 -78%）：
//   1) 源码文件软上限：Rust/TS/TSX/JS 500 行、CSS 3000 行、Python 500 行；
//      SQL 迁移与 i18n 字典、锁文件、生成的类型（*.d.ts / .next）豁免——
//      前者是历史账本（不可变纪律），后者天然单文件、拆分损可读性。
//   2) 存量超限文件进 baseline（file → 当前行数）：允许改、允许变小的重写；
//      只有「比基线更长」才挂——即只许瘦不许胖。
//   3) 新文件超限直接挂（不在 baseline 里且超限）。
// 用法：node scripts/line_limit_guard.mjs [--update]（CI 一步；exit 1 = 违规）。
import { readFileSync, existsSync, writeFileSync } from "node:fs";
import { globSync } from "node:fs";
import path from "node:path";

const ROOT = path.resolve(import.meta.dirname, "..");
const BASELINE = path.resolve(import.meta.dirname, "line_limit_baseline.json");

const LIMITS = {
  ".rs": 500,
  ".ts": 500,
  ".tsx": 500,
  ".js": 500,
  ".mjs": 500,
  ".py": 500,
  ".css": 3000,
};
// 豁免：迁移=不可变账本；i18n 字典/锁/类型生成物/构建产物天然大文件
const EXEMPT = [
  "apps/api/migrations/",
  "apps/web/i18n/",
  "pnpm-lock.yaml",
  "package-lock.json",
  "next-env.d.ts",
  "apps/web/.next/",
  "node_modules/",
  "target/",
  "target-test/",
  "packages/domain-types/", // 共享类型契约：单文件即契约文档，跨包拆分损导航
];

const groups = [
  { cwd: ROOT, globs: ["apps/**/*.*", "packages/**/*.*", "scripts/**/*.*"] },
];
const seen = new Set();
const files = [];
for (const g of groups) {
  for (const pat of g.globs) {
    for (const f of globSync(pat, { cwd: g.cwd, nodir: true })) {
      const rel = f.replace(/\\/g, "/");
      const ext = path.extname(rel);
      if (!(ext in LIMITS)) continue;
      if (EXEMPT.some((p) => rel.startsWith(p) || rel.includes(p))) continue;
      if (!seen.has(rel)) {
        seen.add(rel);
        files.push(rel);
      }
    }
  }
}

const lineCount = (rel) => readFileSync(path.join(ROOT, rel), "utf-8").split("\n").length;

const baseline = existsSync(BASELINE)
  ? JSON.parse(readFileSync(BASELINE, "utf-8"))
  : {};

const offenders = [];
for (const rel of files) {
  const n = lineCount(rel);
  const limit = LIMITS[path.extname(rel)];
  if (n <= limit) continue;
  const was = baseline[rel];
  if (was === undefined) offenders.push({ file: rel, lines: n, limit, kind: "new" });
  else if (n > was) offenders.push({ file: rel, lines: n, limit, was, kind: "grew" });
}

if (process.argv.includes("--update")) {
  const next = {};
  for (const rel of files) {
    const n = lineCount(rel);
    if (n > LIMITS[path.extname(rel)]) next[rel] = n;
  }
  writeFileSync(BASELINE, JSON.stringify(next, null, 1) + "\n");
  console.log(`baseline updated: ${Object.keys(next).length} files over limit`);
  process.exit(0);
}

if (offenders.length) {
  console.error("行数门禁违规（新文件超限 / 存量超限文件变长——只许瘦不许胖）：");
  for (const o of offenders) {
    if (o.kind === "new")
      console.error(`  [新超限] ${o.file} ${o.lines} 行 > 上限 ${o.limit}`);
    else
      console.error(`  [变长] ${o.file} ${o.lines} 行 > 基线 ${o.was} 行（上限 ${o.limit}）`);
  }
  process.exit(1);
}
console.log(
  `行数门禁通过（${files.length} 文件纳入检查，存量超限基线 ${Object.keys(baseline).length} 文件）`,
);
