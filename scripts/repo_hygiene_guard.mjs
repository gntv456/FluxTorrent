// 漏交文件守门（repo_hygiene_guard.mjs）
//
// 背景（两次真实翻车，2026-09-26 与 2026-10-25）：多会话并行开发时，
// 功能 commit 只提交了「引用方」（mod 声明、import、include_bytes!、
// 迁移登记），被引用的文件本体（新模块、新迁移、新拆件）留在未跟踪区。
// 本地容器是从工作区构建的——一切正常；远端 CI / 干净克隆拿到的是
// commit 快照——mod 缺文件 cargo 直接编译失败，迁移断档则首启炸在
// 0001 之后第一个缺口。绿灯本地、红灯远端，排查极费（远端-only 六类
// 挂点之一，见 CI 首绿记忆）。
//
// 检查口径（全部针对 git 快照 vs 工作区的差集，不依赖构建）：
//   1) Rust mod 声明：已跟踪 *.rs 里的 `mod x;`/`#[path=...]` 能否在
//      快照里解析到文件（mod x → x.rs 或 x/mod.rs，含目录链）。
//   2) 迁移连续性：apps/api/migrations 已跟踪编号的最大值 = 全目录
//      （含未跟踪）最大值——有未跟踪迁移就是断档（record_migration/
//      checksum 台账按编号顺序 apply）。
//   3) include_bytes!/include_str! 的字面路径能否解析。
//   4) pnpm workspace 成员 / tsconfig references 的目录存在性。
// 其中 1) 和 2) 覆盖了两次翻车的全部形态；3)4) 是同族顺手收编。
//
// 误报控制：
//   - 只认「快照里缺、工作区里有」的文件——工作区也没有的 mod 是
//     编译期就会暴露的问题，不归本门禁管；
//   - mod 解析只看字面量与蛇形目录约定，不做完整 crate 图分析；
//   - 测试目录的 mod（#[cfg(test)]）同样要求可解析（cargo test 会编）。
//
// 用法：node scripts/repo_hygiene_guard.mjs          # 检查，违规 exit 1
// 本地修复：把红字列出的文件 git add 进对应功能 commit（不是 git add -A
// 一把梭——那会把真正的临时文件也收进去）。
import { execFileSync } from "node:child_process";
import {
  readFileSync, existsSync, readdirSync, statSync,
} from "node:fs";
import path from "node:path";

const ROOT = path.resolve(
  path.dirname(new URL(import.meta.url).pathname.replace(/^\/([A-Za-z]:)/, "$1")),
  "..",
);

function git(args) {
  return execFileSync("git", args, { cwd: ROOT, encoding: "utf-8" });
}

// git 快照里已跟踪的文件集合（HEAD/Index 一致时 ls-files 即快照）
const TRACKED = new Set(git(["ls-files"]).split("\n").filter(Boolean));

// 未跟踪文件（?? 前缀；含未跟踪目录展开后的具体文件）
const UNTRACKED = git(["status", "--porcelain", "-uall", "--no-renames"])
  .split("\n")
  .filter((l) => l.startsWith("?? "))
  .map((l) => l.slice(3).replace(/"/g, "").replace(/\\/g, "/"));

const problems = [];

// ---------- 1) Rust mod 声明可解析 ----------
// 只扫已跟踪的 .rs：引用方已提交、被引用文件没提交才是本门禁的靶。
const MOD_RE = /^\s*(?:pub(?:\(crate\))?\s+)?mod\s+([a-z_][a-z0-9_]*)\s*;/gm;
const PATH_ATTR_RE = /#\[path\s*=\s*"([^"]+)"\]/g;

for (const file of TRACKED) {
  if (!file.endsWith(".rs")) continue;
  let src;
  try {
    src = readFileSync(path.join(ROOT, file), "utf-8");
  } catch {
    continue;
  }
  const dir = path.dirname(file);
  const resolveMod = (name, from) => {
    const cands = [
      path.posix.join(from, `${name}.rs`),
      path.posix.join(from, name, "mod.rs"),
    ];
    // 快照里找得到 → 无问题（工作区多余副本不拦，git 语义如此）
    if (cands.some((c) => TRACKED.has(c))) return;
    // 快照没有、工作区有 → 漏交，正是两次翻车的形态
    const inWt = cands.filter((c) => existsSync(path.join(ROOT, c)));
    if (inWt.length > 0) {
      problems.push(
        `mod 缺文件：${file} 声明 \`mod ${name};\` 但快照里没有——` +
        `工作区存在未跟踪的 ${inWt.join(" / ")}，请随功能 commit 一并 add`,
      );
    }
    // 两边都没有 → cargo build 会红，不属于「本地绿远端红」族，放过
  };

  for (const m of src.matchAll(MOD_RE)) resolveMod(m[1], dir);

  // #[path = "..."] 显式路径：相对声明文件解析
  for (const m of src.matchAll(PATH_ATTR_RE)) {
    const rel = m[1];
    const target = path.posix.normalize(path.posix.join(dir, rel));
    if (!TRACKED.has(target) && existsSync(path.join(ROOT, target))) {
      problems.push(
        `#[path] 缺文件：${file} 引用 "${rel}" → ${target} 未跟踪`,
      );
    }
  }
}

// ---------- 2) 迁移编号断档 ----------
const MIG_DIR = "apps/api/migrations";
const num = (f) => {
  const m = /^(\d+)_/.exec(f);
  return m ? parseInt(m[1], 10) : null;
};
const trackedMigs = [...TRACKED]
  .filter((f) => f.startsWith(`${MIG_DIR}/`))
  .map((f) => num(path.basename(f)))
  .filter((v) => v !== null);
const diskMigs = [];
try {
  for (const f of readdirSync(path.join(ROOT, MIG_DIR))) {
    const v = num(f);
    if (v !== null) diskMigs.push(v);
  }
} catch {
  diskMigs.push(0);
}
const tMax = trackedMigs.length ? Math.max(...trackedMigs) : 0;
const dMax = diskMigs.length ? Math.max(...diskMigs) : 0;
if (dMax > tMax) {
  const missing = diskMigs.filter((v) => v > tMax);
  problems.push(
    `迁移断档：${MIG_DIR} 磁盘最大编号 ${dMax}，快照最大 ${tMax}——` +
    `编号 ${missing.join(", ")} 的迁移文件未提交（checksum 台账按序 apply，` +
    `干净环境首启会炸在断档处）`,
  );
}
// 快照内部断档也顺带查——但历史「撞号让号」的合法空洞（编号被并行会话
// 占用过、后让号给新迁移，台账 _sqlx_migrations 同样跳过）登记在基线里
// 不报：128/129/130（0142 换号回退）、202（土地阶梯让号）、268/269
// （0332 让号 0333 族）、362（并行撞号）。新空洞不在基线 → 红。
const KNOWN_GAPS = new Set([129, 130, 202, 268, 269, 362]);
const tSorted = trackedMigs.sort((a, b) => a - b);
for (let i = 1; i < tSorted.length; i++) {
  if (tSorted[i] > tSorted[i - 1] + 1) {
    const gap = [];
    for (let v = tSorted[i - 1] + 1; v < tSorted[i]; v++) {
      if (!diskMigs.includes(v) && !KNOWN_GAPS.has(v)) gap.push(v);
    }
    if (gap.length > 0) {
      problems.push(
        `迁移真空：${MIG_DIR} 编号 ${gap.join(",")} 快照与工作区都不存在` +
        `（${tSorted[i - 1]} 与 ${tSorted[i]} 之间；历史让号空洞见 KNOWN_GAPS）`,
      );
    }
  }
}

// ---------- 3) include_bytes!/include_str! 字面路径 ----------
const INC_RE = /include_(?:bytes|str)!\(\s*"([^"]+)"/g;
for (const file of TRACKED) {
  if (!file.endsWith(".rs")) continue;
  let src;
  try {
    src = readFileSync(path.join(ROOT, file), "utf-8");
  } catch {
    continue;
  }
  for (const m of src.matchAll(INC_RE)) {
    // 这类引用多带 build 期拼接（target/ 产物），只查裸字面量可解析的
    const target = path.posix.normalize(
      path.posix.join(path.dirname(file), m[1]),
    );
    if (target.includes("target/")) continue; // 构建产物，cargo 自己管
    if (!TRACKED.has(target) && existsSync(path.join(ROOT, target))) {
      problems.push(
        `include_*! 缺文件：${file} 引用 "${m[1]}" → ${target} 未跟踪`,
      );
    }
  }
}

// ---------- 4) workspace / tsconfig 引用目录 ----------
try {
  const pw = JSON.parse(readFileSync(path.join(ROOT, "pnpm-workspace.yaml"), "utf-8").replace(/^packages:[\s\S]*$/m, (s) => s) || "{}");
  void pw; // yaml 目录通配解析复杂，改走轻口径：tsconfig references
} catch { /* 无该文件则跳过 */ }
for (const file of TRACKED) {
  if (!/(^|\/)tsconfig\.json$/.test(file) || file.includes("node_modules")) continue;
  let cfg;
  try {
    cfg = JSON.parse(
      readFileSync(path.join(ROOT, file), "utf-8")
        .replace(/\/\/[^\n]*/g, ""),
    );
  } catch {
    continue;
  }
  for (const ref of cfg.references ?? []) {
    const dir = path.posix.normalize(
      path.posix.join(path.dirname(file), ref.path || ""),
    );
    const hasAny = [...TRACKED].some((t) => t.startsWith(dir));
    const wtHas = existsSync(path.join(ROOT, dir));
    if (!hasAny && wtHas) {
      problems.push(
        `tsconfig reference 缺目录：${file} 引用 ${ref.path} → ${dir} 未跟踪`,
      );
    }
  }
}

// ---------- 汇总 ----------
if (problems.length > 0) {
  console.error(`repo_hygiene_guard：${problems.length} 处「引用已提交、本体未 add」`);
  for (const p of problems) console.error("  - " + p);
  console.error(
    "\n修复：把上述文件 git add 进对应功能 commit。" +
    "（勿 git add -A 一把梭——临时文件会混入）",
  );
  process.exit(1);
}
console.log(
  `repo_hygiene_guard PASS（mod/迁移/include/tsconfig 四口径；` +
  `未跟踪 ${UNTRACKED.length} 个文件均无被引用）`,
);
