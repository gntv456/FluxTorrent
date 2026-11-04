// 迁移冻结门禁（2026-11-04）：已入库迁移文件**一字节都不许改，注释也算**。
//
// 背景：2026-11-03 主题批次把 0448 的一行注释 `--brand-paper` 改成 `--compat-paper`，
// 文件字节变化 → sqlx 的 SHA-384 checksum 从 38636f… 变成 029773… → 任何「改写前」
// 镜像对存量库启动即报 `migration 448 was previously applied but has been modified`。
// 这类改动 SQL 语义往往没变，只有校验和变了，review 时极易漏看，必须在合并前静态拦住。
//
// 与 scripts/audit_migration_checksums.py 的分工：
//   - audit_migration_checksums.py：运行时审计，用 **git blob 原始字节** 对拍
//     `_sqlx_migrations`（需要起 postgres），管「库 ↔ 仓库」口径。
//   - 本脚本：合并前静态冻结，用 **基线哈希** 对拍已入库文件（不需要 DB），
//     管「仓库 ↔ 上次冻结」口径；两者互补，不重复。
//
// 哈希口径（与 sqlx 对齐）：
//   sqlx 存 SHA-384（对构建时的文件原始字节）。仓库 `.gitattributes` 已把 `*.sql`
//   锁为 LF，因此本脚本先做 CRLF→LF 归一化再算 SHA-384；对 LF 检出的文件，
//   该值与 `_sqlx_migrations.checksum` 完全一致。另存一个「去注释 + 去缩进/空行」
//   的语义哈希，仅用于把「只改注释/空白」和「真改 SQL」区分开，不作为放行依据。
//
// 用法：
//   node scripts/migrations_frozen_guard.mjs                 # 校验（CI / 本地）
//   node scripts/migrations_frozen_guard.mjs --init          # 仅首次建立基线
//   node scripts/migrations_frozen_guard.mjs --touch <file>  # 登记/重算单个文件
//   node scripts/migrations_frozen_guard.mjs --help
// 测试用覆盖项（正常使用不要传）：
//   --dir <迁移目录>  --baseline <基线文件>
//
// 退出码：0 通过（含“有未登记新迁移”的正常放行）/ 1 失败。
import { createHash } from "node:crypto";
import { existsSync, readFileSync, readdirSync, writeFileSync } from "node:fs";
import path from "node:path";

const SCRIPT_DIR = import.meta.dirname;
const REPO_ROOT = path.resolve(SCRIPT_DIR, "..");
const DEFAULT_DIR = path.join(REPO_ROOT, "apps/api/migrations");
const DEFAULT_BASELINE = path.join(SCRIPT_DIR, "migrations_frozen_baseline.json");
const MIGRATION_NAME = /^\d{4}_.*\.sql$/;

function usage() {
  console.log(`用法：
  node scripts/migrations_frozen_guard.mjs                 校验（默认）
  node scripts/migrations_frozen_guard.mjs --init          首次建立基线（已存在则拒绝）
  node scripts/migrations_frozen_guard.mjs --touch <file>  登记新增迁移 / 显式重算单个文件
  --help                                                   显示本帮助`);
}

function parseArgs(argv) {
  const out = { mode: "check", dir: DEFAULT_DIR, baseline: DEFAULT_BASELINE, files: [] };
  for (let i = 0; i < argv.length; i++) {
    const a = argv[i];
    if (a === "--help" || a === "-h") out.mode = "help";
    else if (a === "--init") out.mode = "init";
    else if (a === "--touch" || a === "--add") {
      out.mode = "touch";
      while (i + 1 < argv.length && !argv[i + 1].startsWith("--")) out.files.push(argv[++i]);
    } else if (a === "--dir") out.dir = path.resolve(argv[++i]);
    else if (a === "--baseline") out.baseline = path.resolve(argv[++i]);
    else {
      console.error(`未知参数：${a}`);
      usage();
      process.exit(1);
    }
  }
  return out;
}

function sha384(text) {
  return createHash("sha384").update(Buffer.from(text, "utf8")).digest("hex");
}

/** SQL 注释剥离（支持 '…' / "…" 字符串，`--` 行注释、块注释；$$ 体不展开，仅作提示用） */
function stripSqlComments(sql) {
  let out = "";
  let state = "normal";
  for (let i = 0; i < sql.length; ) {
    const c = sql[i];
    const n = sql[i + 1];
    if (state === "normal") {
      if (c === "-" && n === "-") {
        while (i < sql.length && sql[i] !== "\n") i++;
        continue;
      }
      if (c === "/" && n === "*") {
        i += 2;
        while (i < sql.length && !(sql[i] === "*" && sql[i + 1] === "/")) i++;
        i += 2;
        continue;
      }
      if (c === "'") state = "single";
      else if (c === '"') state = "double";
      out += c;
      i++;
      continue;
    }
    out += c;
    if (state === "single" && c === "'") {
      if (sql[i + 1] === "'") {
        out += "'";
        i += 2;
        continue;
      }
      state = "normal";
    } else if (state === "double" && c === '"') {
      state = "normal";
    }
    i++;
  }
  return out;
}

/** 语义哈希：去注释 + 去行首尾空白 + 去空行（字符串内部空白保留） */
function entryOf(file) {
  const raw = readFileSync(file, "utf8").replace(/\r\n/g, "\n");
  const code = stripSqlComments(raw)
    .split("\n")
    .map((l) => l.trim())
    .filter(Boolean)
    .join("\n");
  return { sha384: sha384(raw), codeSha384: sha384(code) };
}

function listMigrations(dir) {
  return readdirSync(dir)
    .filter((f) => MIGRATION_NAME.test(f))
    .sort();
}

function loadBaseline(file) {
  if (!existsSync(file)) return null;
  const parsed = JSON.parse(readFileSync(file, "utf8"));
  // 兼容最简形态（文件名 → sha384 字符串）：语义哈希未知时按“内容变化”处理
  const out = {};
  for (const [k, v] of Object.entries(parsed)) {
    out[k] = typeof v === "string" ? { sha384: v, codeSha384: null } : v;
  }
  return out;
}

function writeBaseline(file, map) {
  const sorted = {};
  for (const k of Object.keys(map).sort()) sorted[k] = map[k];
  writeFileSync(file, JSON.stringify(sorted, null, 1) + "\n");
}

function main() {
  const args = parseArgs(process.argv.slice(2));
  if (args.mode === "help") {
    usage();
    return 0;
  }
  if (!existsSync(args.dir)) {
    console.error(`迁移目录不存在：${args.dir}`);
    return 1;
  }
  const files = listMigrations(args.dir);

  if (args.mode === "init") {
    if (existsSync(args.baseline)) {
      console.error(`基线已存在，拒绝覆盖：${args.baseline}`);
      console.error("新增迁移请用 --touch <file>；确需重建基线请先人工确认并删除旧文件。");
      return 1;
    }
    const map = {};
    for (const f of files) map[f] = entryOf(path.join(args.dir, f));
    writeBaseline(args.baseline, map);
    console.log(`已建立迁移冻结基线：${args.baseline}（${files.length} 个文件）`);
    return 0;
  }

  if (args.mode === "touch") {
    if (!args.files.length) {
      console.error("--touch 需要至少一个迁移文件名");
      return 1;
    }
    const baseline = loadBaseline(args.baseline) ?? {};
    for (const name of args.files) {
      if (!MIGRATION_NAME.test(name)) {
        console.error(`不是合法迁移文件名：${name}`);
        return 1;
      }
      const full = path.join(args.dir, name);
      if (!existsSync(full)) {
        console.error(`迁移文件不存在：${full}`);
        return 1;
      }
      const existed = name in baseline;
      baseline[name] = entryOf(full);
      console.log(existed
        ? `WARN: 已重新登记 ${name}（覆盖冻结值；仅限维护者显式操作）`
        : `已登记新迁移 ${name}`);
    }
    writeBaseline(args.baseline, baseline);
    return 0;
  }

  // check
  const baseline = loadBaseline(args.baseline);
  if (!baseline) {
    console.error(`基线文件不存在：${args.baseline}`);
    console.error("首次请运行 node scripts/migrations_frozen_guard.mjs --init");
    return 1;
  }

  const errors = [];
  const fresh = [];
  for (const name of files) {
    const cur = entryOf(path.join(args.dir, name));
    const was = baseline[name];
    if (!was) {
      fresh.push(name);
      continue;
    }
    if (was.sha384 !== cur.sha384) {
      const commentOnly = was.codeSha384 && was.codeSha384 === cur.codeSha384;
      errors.push({ name, was, cur, commentOnly });
    }
  }
  for (const name of Object.keys(baseline)) {
    if (!files.includes(name)) errors.push({ name, deleted: true });
  }

  if (fresh.length) {
    console.log(`WARN: ${fresh.length} 个未登记的新迁移（正常放行；入库时请显式登记冻结值）：`);
    for (const name of fresh) console.log(`  - ${name}`);
    console.log(`    登记：node scripts/migrations_frozen_guard.mjs --touch ${fresh.join(" ")}`);
  }

  if (errors.length) {
    console.error(`FAIL: migrations frozen 门禁失败（${errors.length} 处）：`);
    for (const e of errors) {
      if (e.deleted) {
        console.error(`  - ${e.name}`);
        console.error("    迁移文件被删除：已应用迁移缺失会让 sqlx 报 missing，禁止删除。");
        continue;
      }
      console.error(`  - ${e.name}`);
      console.error(`    baseline sha384: ${e.was.sha384}`);
      console.error(`    current  sha384: ${e.cur.sha384}`);
      console.error(e.commentOnly
        ? "    摘要: 仅注释/空白变化（SQL 语义哈希未变）——仍禁止：sqlx checksum 会漂移，镜像将拒启。"
        : "    摘要: SQL 语句本身发生变化（高风险）——请改为新建迁移，不要改历史文件。");
      console.error(`    复核：git log -p -- apps/api/migrations/${e.name}`);
    }
    console.error("确属新增迁移请用 --touch 登记；历史迁移的任何改动都应回滚。");
    return 1;
  }

  console.log(`PASS: migrations frozen 门禁通过（已登记 ${Object.keys(baseline).length} 个，未登记新迁移 ${fresh.length} 个）`);
  return 0;
}

process.exit(main());
