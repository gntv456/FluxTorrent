// i18n 防新增门禁脚本（审查路线图第 4 周：先堵新增，存量分批迁移）。
// 规则：components/ 与 app/ 下的 ts/tsx 文件不允许新增含 CJK 的 JSX 文本节点
// 或字符串字面量（已有存量见 scripts/i18n_baseline.json；翻译请进 i18n/zh-CN.ts）。
// 例外：i18n/ 目录本身、tests/、*.d.ts、注释行、console/tracing 类调试输出不做要求。
// 用法：node scripts/i18n_guard.mjs（CI 中作为 web job 一步；exit 1 = 有新增）。
//
// ⚠️ 基线按**行号**记录，任何行增删都会让存量文案被判成「新增」。
// 迁移某个文件后**不要**跑 `--update`（它会把并行会话的新违规一并洗白），
// 改用 `scripts/i18n_baseline_touch.mjs <文件...>` 只重算指定文件。
import { readFileSync, existsSync, writeFileSync } from "node:fs";
import { globSync } from "node:fs";
import path from "node:path";

const ROOT = path.resolve(import.meta.dirname, "..", "apps/web");
const BASELINE = path.resolve(import.meta.dirname, "i18n_baseline.json");

const CJK = /[\u4e00-\u9fff]/;
// ⚠️ 必须同时扫 .ts —— 管理端相当一部分文案不在 tsx 里，而在
// `admin-*-shared.ts`（选项表 / 分组字典 / 默认值）这类纯 ts 模块中。
// 只扫 tsx 会漏掉这 154 条（实测），门禁形同虚设。排除 .d.ts / i18n / tests。
const GLOBS = ["**/*.ts", "**/*.tsx"];
const files = GLOBS.flatMap((p) => globSync(p, { cwd: ROOT })).filter(
  (f) =>
    !f.includes("node_modules") &&
    !f.includes(".next") &&
    !f.replace(/\\/g, "/").includes("i18n/") &&
    !f.replace(/\\/g, "/").includes("tests/") &&
    !f.endsWith(".d.ts"),
);

/** 提取文件级命中行号集合（JSX 文本 + 字符串字面量，忽略注释行） */
function hitsOf(file) {
  const lines = readFileSync(path.join(ROOT, file), "utf-8").split("\n");
  const out = [];
  lines.forEach((l, i) => {
    const t = l.trim();
    if (t.startsWith("//") || t.startsWith("*") || t.startsWith("/*")) return;
    const jsx = l.match(/>\s*([^<>{}\n]*[\u4e00-\u9fff][^<>{}\n]*)\s*</);
    const str = l.match(/["'`]([^"'`\n]*[\u4e00-\u9fff][^"'`\n]*)["'`]/);
    if (jsx || str) out.push(i + 1);
  });
  return out;
}

const baseline = existsSync(BASELINE)
  ? JSON.parse(readFileSync(BASELINE, "utf-8"))
  : {};

let regressions = [];
for (const f of files) {
  const cur = hitsOf(f);
  const was = baseline[f] ?? [];
  const fresh = cur.filter((n) => !was.includes(n));
  if (fresh.length) regressions.push({ file: f, lines: fresh });
}

if (process.argv.includes("--update")) {
  const next = {};
  for (const f of files) next[f] = hitsOf(f);
  writeFileSync(BASELINE, JSON.stringify(next, null, 1));
  console.log(`baseline updated: ${Object.keys(next).length} files`);
  process.exit(0);
}

if (regressions.length) {
  console.error("i18n 门禁：以下位置新增了硬编码中文（请移入 i18n/zh-CN.ts 并三语同步）：");
  for (const r of regressions) console.error(`  ${r.file}:${r.lines.join(",")}`);
  process.exit(1);
}
console.log(`i18n 门禁通过（${files.length} 文件，存量基线 ${Object.keys(baseline).length} 文件）`);
