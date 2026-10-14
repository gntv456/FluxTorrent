/**
 * 审计 action 命名门禁（FRAME-PLAN-01 §7 第 9 条）：规约统一且 lint 可查。
 *
 * 规约（本批立例）：audit_log.action 一律**点分层 `域.动作[.对象]`**
 * （如 user.passkey.reset、torrent.batch.delete）——可排序、可前缀检索、
 * 与 OpenAPI tag/路由域同构。下划线蛇形（passkey_rotate）与冒号形
 * （setting:update）是历史存量。
 *
 * 现状摸底（2026-10-13，扫描 apps/api/src/**.rs 的 .audit( / .audit_detail(
 * 调用首个字符串字面量）：311 个唯一 action，其中点分层 211、下划线层 94、
 * 冒号形 6、误捕 1（"id"，见下）。
 *
 * 存量策略——**只防新增劣化，不改存量值**：
 *   · action 已写进 audit_log 链，改值 = 破坏既有检索/对账的延续性
 *     （audit_chain_verify 按行校验，历史行不重写）；
 *   · 下划线/冒号存量登记进 baseline json 棘轮：只许降不许升；
 *   · 新 action（不在 baseline 的）违规模式直接拦，零容忍。
 *
 * 违规定义（拦新 + 棘轮之外的存量全查）：
 *   ① 连续下划线段 ≥2（a_b_c 或 a_b）；
 *   ② 含中文或空格；
 *   ③ 冒号形（域:动作）——与点分层并存的第三流派，同受棘轮约束。
 *
 * 误捕处理："id"、"target_user"、"name" 这类字段名字面量可能出现在
 * audit( 调用后的窗口里（跨参数捕获）。因此捕获窗口限定为「调用起括号后
 * 到第一个字符串字面量」，且 baseline 里登记了已知的非 action 误捕项
 * （"id"）跳过不查。
 *
 * 用法：node scripts/audit_action_guard.mjs [--update]
 *   --update 重新生成 baseline（迁移/删除存量 action 后收缩棘轮用）
 *   exit 0=通过 1=违规
 * 红测口径：临时在某 handler 加 .audit(Some(u), "some_bad_name", None)
 * → ①命中拦下 exit 1；还原后复绿。
 */
import { readFileSync, writeFileSync, existsSync } from "node:fs";
import { readdirSync, statSync } from "node:fs";
import path from "node:path";
import { fileURLToPath } from "node:url";

const ROOT = path.resolve(path.dirname(fileURLToPath(import.meta.url)), "..");
const SRC = path.join(ROOT, "apps", "api", "src");
const BASELINE = path.join(
  path.dirname(fileURLToPath(import.meta.url)),
  "audit_action_baseline.json",
);

// 已知非 action 的误捕字面量（audit 调用窗口内的字段名等）——跳过
const NOT_ACTIONS = new Set(["id", "target_user", "name", "reason"]);

function walk(dir, out) {
  for (const e of readdirSync(dir)) {
    const p = path.join(dir, e);
    const st = statSync(p);
    if (st.isDirectory()) walk(p, out);
    else if (e.endsWith(".rs")) out.push(p);
  }
  return out;
}

/**
 * 提取所有 .audit( / .audit_detail( 调用的首个字符串字面量。
 * 窗口：调用起括号起 300 字符内的第一个 "..."——覆盖换行/多参数形态；
 * format! 动态串（如 "torrent.batch.{}"）按字面量模板计。
 */
function extractActions() {
  const found = []; // { file, action }
  for (const rel of walk(SRC, []).map((f) => path.relative(ROOT, f))) {
    const src = readFileSync(path.join(ROOT, rel), "utf8");
    const re = /\.audit(?:_detail)?\s*\(/g;
    let m;
    while ((m = re.exec(src))) {
      const win = src.slice(m.index, m.index + 300);
      const lit = win.match(/"([^"]*)"/);
      if (!lit) continue;
      found.push({ file: rel.replace(/\\/g, "/"), action: lit[1] });
    }
  }
  return found;
}

const isUnderscore = (a) => /^[a-z0-9]+(_[a-z0-9]+)+$/.test(a);
const isColon = (a) => /^[a-z0-9_]+:[a-z0-9_]+$/.test(a);
const hasCJK = (a) => /[\u4e00-\u9fff]/.test(a);
const hasSpace = (a) => /\s/.test(a);
const isDotted = (a) => /^[a-z0-9_{}]+(\.[a-z0-9_{}]+)+$/.test(a);

// 违规判定：点分层（含 {} 模板位）之外全是违规形态
const violates = (a) =>
  !isDotted(a) && (isUnderscore(a) || isColon(a) || hasCJK(a) || hasSpace(a));

const actions = extractActions().filter((x) => !NOT_ACTIONS.has(x.action));
const unique = [...new Set(actions.map((x) => x.action))];

const baseline = existsSync(BASELINE)
  ? JSON.parse(readFileSync(BASELINE, "utf8"))
  : { legacy_underscore_or_colon: [] };
const base = new Set(baseline.legacy_underscore_or_colon);

let fails = 0;
const section = (t) => console.log(`\n== ${t} ==`);

section("action 形态统计");
const nDotted = unique.filter(isDotted).length;
const nLegacy = unique.filter(violates).length;
console.log(
  `unique=${unique.length} 点分层=${nDotted} 下划线/冒号=${nLegacy}` +
    `（baseline=${base.size}）`,
);

section("新文件零容忍：baseline 外的违规 action");
const newBad = unique.filter((a) => violates(a) && !base.has(a));
if (newBad.length) {
  for (const a of newBad) {
    const where = actions.filter((x) => x.action === a).map((x) => x.file);
    console.log(`FAIL  新增违规 action "${a}"（${[...new Set(where)].join(", ")}）`);
    fails++;
  }
} else {
  console.log("OK  无 baseline 外新增违规");
}

section("存量棘轮：baseline 内 action 仍被使用（只许降）");
const stillUsed = [...base].filter((a) => unique.includes(a));
const dropped = [...base].filter((a) => !unique.includes(a));
if (dropped.length) {
  console.log(
    `OK  ${dropped.length} 个 baseline 项已不再使用，可 --update 收缩：` +
      `${dropped.join(", ")}`,
  );
}
const nextBaseline = {
  _doc: "存量下划线/冒号形 audit action 棘轮（FRAME §7 第 9 条）：只许降不许升；迁移点分层后跑 --update 收缩",
  legacy_underscore_or_colon: stillUsed.sort(),
};

section("非 action 误捕核对（NOT_ACTIONS 白名单仍有效）");
for (const w of NOT_ACTIONS) {
  // 若白名单项实际不再出现在捕获里，提示可清理（不阻断）
  if (!actions.some((x) => x.action === w)) {
    console.log(`WARN  NOT_ACTIONS 白名单项 "${w}" 已无捕获，可清理`);
  }
}

if (process.argv.includes("--update")) {
  // 首建 baseline：把当前全部违规形态（含尚未在册的）登记为存量；
  // 已有 baseline 时只收缩（stillUsed），不扩——防 --update 被用来洗白新增
  const seed = base.size === 0
    ? unique.filter(violates).sort()
    : nextBaseline.legacy_underscore_or_colon;
  writeFileSync(
    BASELINE,
    JSON.stringify(
      {
        _doc: nextBaseline._doc,
        legacy_underscore_or_colon: seed,
      },
      null,
      2,
    ) + "\n",
  );
  console.log(`\nbaseline 已更新：${seed.length} 项`);
  process.exit(0);
}

console.log(
  fails === 0
    ? `\nAUDIT_ACTION_OK（点分层规约：域.动作[.对象]）`
    : `\nAUDIT_ACTION_FAIL ${fails} 处违规（新 action 须点分层，如 user.passkey.reset）`,
);
process.exit(fails === 0 ? 0 : 1);
