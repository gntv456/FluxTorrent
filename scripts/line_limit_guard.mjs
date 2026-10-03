// 文件行数与行宽门禁（防膨胀）：新文件不许超限，存量超限文件不许再变大。
// 规则（对齐团队「按域拆分」的既有实践，如 http.rs 六段外移 -78%）：
//   1) 源码文件软上限：Rust/TS/TSX/JS 300 行、CSS 3000 行、Python 500 行；
//      SQL 迁移与 i18n 字典、锁文件、生成的类型（*.d.ts / .next）豁免——
//      前者是历史账本（不可变纪律），后者天然单文件、拆分损可读性。
//   2) 存量超限文件进 baseline（file → 当前行数）：允许改、允许变小的重写；
//      只有「比基线更长」才挂——即只许瘦不许胖。
//   3) 新文件超限直接挂（不在 baseline 里且超限）。
//   4) 行宽 ≤80 字符（含 UTF-8 中文字符按 1 计）：超标算文件违规行数，
//      新文件/基线内文件不许超标；基线文件以「超宽行数不增」为准——
//      老文件可以改，但不许把超宽行改得更多。
// 用法：node scripts/line_limit_guard.mjs [--update]（CI 一步；exit 1 = 违规）。
import { readFileSync, existsSync, writeFileSync } from "node:fs";
import { globCompat as globSync } from "./lib/glob_compat.mjs";
import path from "node:path";

const ROOT = path.resolve(import.meta.dirname, "..");
const BASELINE = path.resolve(import.meta.dirname, "line_limit_baseline.json");

const LIMITS = {
  ".rs": 300,
  ".ts": 300,
  ".tsx": 300,
  ".js": 300,
  ".mjs": 300,
  ".py": 500,
  ".css": 3000,
};
const MAX_COLS = 80;
// 豁免：迁移=不可变账本；i18n 字典/锁/类型生成物/构建产物天然大文件
const EXEMPT = [
  "apps/api/migrations/",
  "apps/web/i18n/",
  "pnpm-lock.yaml",
  "package-lock.json",
  "next-env.d.ts",
  "apps/web/.next/",
  "coverage/", // 覆盖率报告是生成物且已 gitignore，不是源码
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

// ⚠️ 必须 strip 行尾 \r：仓库 core.autocrlf=true，工作区是 CRLF。按 \n 切分后
// 每行会多出一个 \r，恰好卡在 80 的行会被误判为超宽 —— 同一文件在 LF/CRLF
// 工作区之间切换会凭空触发「超宽变多」。行尾回车不是可见列，不算宽度。
const readLines = (rel) =>
  readFileSync(path.join(ROOT, rel), "utf-8")
    .split("\n")
    .map((l) => l.replace(/\r$/, ""));
const wideLines = (lines) => lines.filter((l) => l.length > MAX_COLS).length;

const baseline = existsSync(BASELINE)
  ? JSON.parse(readFileSync(BASELINE, "utf-8"))
  : {};

const offenders = [];
const nextBaseline = {};
for (const rel of files) {
  const lines = readLines(rel);
  const n = lines.length;
  const limit = LIMITS[path.extname(rel)];
  const was = baseline[rel];

  // 行数：超限进 next 基线；比旧基线更长即挂（新文件=不在基线且超限）
  if (n > limit) {
    nextBaseline[rel] = n;
    if (was === undefined) {
      offenders.push({ file: rel, lines: n, limit, kind: "new" });
    } else if (n > was) {
      offenders.push({ file: rel, lines: n, limit, was, kind: "grew" });
    }
  }

  // 行宽：与行数同款「只许瘦不许胖」——基线登记超宽行数，比登记多即挂；
  // 没登记的文件（新建 / 原本干净）出现任何超宽行直接挂。
  const wide = wideLines(lines);
  if (wide > 0) {
    const wasWide = baseline[rel + "::wide"];
    if (wasWide === undefined) {
      offenders.push({ file: rel, wide, kind: "new-wide" });
    } else if (wide > wasWide) {
      offenders.push({ file: rel, wide, wasWide, kind: "grew-wide" });
    }
    nextBaseline[rel + "::wide"] = wide;
  }
}

if (process.argv.includes("--update")) {
  writeFileSync(BASELINE, JSON.stringify(nextBaseline, null, 1) + "\n");
  const filesOver = Object.keys(nextBaseline).filter(
    (k) => !k.endsWith("::wide"),
  ).length;
  console.log(
    `baseline updated: ${filesOver} files over limit, ` +
      `${Object.keys(nextBaseline).length - filesOver} files with wide lines`,
  );
  process.exit(0);
}

if (offenders.length) {
  console.error(
    "行数/行宽门禁违规（新文件超限 / 存量超限只许瘦不许胖 / 行宽≤80）：",
  );
  for (const o of offenders) {
    if (o.kind === "new")
      console.error(
        `  [新超限] ${o.file} ${o.lines} 行 > 上限 ${o.limit}`,
      );
    else if (o.kind === "grew")
      console.error(
        `  [变长] ${o.file} ${o.lines} 行 > 基线 ${o.was} 行（上限 ${o.limit}）`,
      );
    else if (o.kind === "new-wide")
      console.error(
        `  [新文件超宽] ${o.file} ${o.wide} 行 > ${MAX_COLS} 字符`,
      );
    else
      console.error(
        `  [超宽变多] ${o.file} ${o.wide} 行 > 基线 ${o.wasWide} 行` +
          `（> ${MAX_COLS} 字符）`,
      );
  }
  process.exit(1);
}
console.log(
  `行数/行宽门禁通过（${files.length} 文件纳入检查，` +
    `超限/超宽基线 ${Object.keys(baseline).length} 条）`,
);
