// 错误码注册表生成器（W-13 / FRAME-PLAN-01 F1 步②）：
//   errors.rs `code()` 的变体→码→触发条件注释 ↔ i18n.rs MESSAGES 三语文案
//   join 成注册表，产出两份生成物（确定性输出，无时间戳）：
//     1) docs/api/error-codes.md                人类可读注册表（含 HTTP 态）
//     2) packages/domain-types/src/error-codes.generated.ts
//        export const ERROR_CODES（变体名 SCREAMING_SNAKE）——F4 消费端
//        才替换 index.ts 手写 ErrorCode；本步只生成并在 index.ts 末尾 re-export。
//
// 对拍语义（--check）：FAIL 只在**脚本抓不到源**（映射缺注释=文档出现空
// 触发条件、解析器漏抓 i18n 条目）；码集多/缺由 check_type_drift.mjs 第 4 维
// 对拍揭示（WARN 不阻断——本步是揭示不是阻断）。i18n 已知缺文案的码
// （4102/5003，见 error_registry.mjs KNOWN_GAPS）列出待补清单不阻塞。
//
// 与 error_registry.mjs 的分工：那边是对拍门禁（三方一致性 + 文档过期即红，
// 仍生效不动）；本脚本把注册表**升级**为带触发条件与 HTTP 态的四维版本，
// 输出文件名相同（docs/api/error-codes.md 被本脚本接管）。
//
// 用法：
//   node scripts/error_codes_gen.mjs            # 生成/刷新（--update 同义）
//   node scripts/error_codes_gen.mjs --check    # 生成物过期 → exit 1
import { readFileSync, writeFileSync, existsSync } from "node:fs";
import path from "node:path";

const ROOT = path.resolve(import.meta.dirname, "..");
const OUT_MD = path.join(ROOT, "docs", "api", "error-codes.md");
const OUT_TS = path.join(
  ROOT, "packages", "domain-types", "src", "error-codes.generated.ts",
);
const ERRORS_RS = "apps/api/src/errors.rs";
const I18N_RS = "apps/api/src/i18n.rs";
// i18n 存量缺文案（error_registry.mjs KNOWN_GAPS 同口径）：列出待补不阻塞
const KNOWN_GAPS = new Set([4102, 5003]);

const read = (p) => readFileSync(path.join(ROOT, p), "utf8");

// ---- ① errors.rs：code() 映射（变体→码→单行触发条件注释） ----
// 注释格式（errors.rs code() 内逐臂）：`// <code> <变体>：触发条件`
const errSrc = read(ERRORS_RS);
const codeFn = errSrc.match(/fn code\(&self\) -> i32 \{([\s\S]*?)\n    \}/)?.[1]
  ?? "";

// 变体→码：match 臂（dtor 形态 `Variant(_)` / 无参 `Variant` 都抓）
const variantCode = new Map(); // variant -> code
for (const m of codeFn.matchAll(
  /DomainError::(\w+)(?:\([^)]*\))?\s*=>\s*(\d+)/g,
)) {
  variantCode.set(m[1], Number(m[2]));
}
// 码→触发条件注释：`// <code> <变体>：条件`（逐臂一行，多臂同码如 1002
// 两条注释都算数——文档里用 <br> 并列）。不匹配任何臂的注释跳过（如分段说明）。
const codeTrigger = new Map(); // code -> [conditions]
for (const m of codeFn.matchAll(/\/\/ (\d+) (\w+)：(.+)/g)) {
  const code = Number(m[1]);
  const variant = m[2];
  // 注释里的变体名必须真实映射到该码，防游离注释混进注册表
  if (variantCode.get(variant) !== code) continue;
  if (!codeTrigger.has(code)) codeTrigger.set(code, []);
  codeTrigger.get(code).push(m[3].trim());
}

// ---- ①b errors.rs：status() 映射（变体→HTTP StatusCode 常量 → 数字态） ----
const statusFn = errSrc.match(
  /fn status\(&self\) -> StatusCode \{([\s\S]*?)\n    \}/,
)?.[1] ?? "";
const HTTP_STATUS = {
  OK: 200, BAD_REQUEST: 400, UNAUTHORIZED: 401, FORBIDDEN: 403,
  NOT_FOUND: 404, CONFLICT: 409, TOO_MANY_REQUESTS: 429,
  INTERNAL_SERVER_ERROR: 500, SERVICE_UNAVAILABLE: 503,
};
// status() 的臂形态与 code() 不同：或臂（`A | B | C => ...`，跨行续臂无
// DomainError 前缀）与块臂（`X => { StatusCode::Y }`）都抓——把 match 函数
// 体按顶层 `=>` 切成臂序列，每臂是「自上一个 => 之后的全部文本」，其中
// 最后一次出现的 DomainError:: 变体列表（含续臂裸名 `| TwoFactorInvalid`）
// 属于本臂、其后的 StatusCode:: 是本臂状态。先做括号深度扫描保护块臂的
// `{ StatusCode::CONFLICT }`（避免块内出现的记号参与切分）。
const variantStatus = new Map(); // variant -> "NNN 常量名"
{
  // statusFn 含 match 自己的开括号——先剥掉，否则整段都在深度 1 里
  const body = statusFn.slice(statusFn.indexOf("{") + 1);
  // 深度标注：本仓 `(...)` 只出现在变体的 dtor 形态（`NotFound(_)`），
  // 块臂的 `{ ... }` 需要保护；dtor 括号若也算深度会把臂内 `=>` 误标成
  // 嵌套——因此只对 `{` `}` 计深度。
  const marked = new Array(body.length).fill(false);
  {
    let depth = 0;
    for (let i = 0; i < body.length; i++) {
      const ch = body[i];
      if (ch === "{") depth++;
      if (depth > 0) marked[i] = true;
      if (ch === "}") depth--;
    }
  }
  // 顶层 `=>` 位点：split 的等价物，但只认深度 0 的
  const cuts = [];
  for (let i = 0; i + 1 < body.length; i++) {
    if (!marked[i] && !marked[i + 1]
      && body.slice(i, i + 2) === "=>") {
      cuts.push(i);
    }
  }
  for (let c = 0; c < cuts.length; c++) {
    // 臂 lhs 起点：上一个顶层 => 的结束（首个臂从 0 起）
    const lhsStart = c === 0 ? 0 : cuts[c - 1] + 2;
    const lhs = body.slice(lhsStart, cuts[c]);
    // 或臂的续臂在本仓里全是全限定名（`| DomainError::X`），matchAll 直抓
    const variants = [...lhs.matchAll(/DomainError::(\w+)/g)].map((m) => m[1]);
    if (!variants.length) continue; // 尾注/杂项片段，非变体臂
    // rhs：到下一个顶层 => 或块臂结束，抓首个 StatusCode 常量（深度 0/块内都行）
    const rhsEnd = c + 1 < cuts.length ? cuts[c + 1] : body.length;
    const rhs = body.slice(cuts[c] + 2, rhsEnd);
    const name = rhs.match(/StatusCode::(\w+)/)?.[1];
    if (!name || !(name in HTTP_STATUS)) continue;
    for (const v of variants) {
      variantStatus.set(v, `${HTTP_STATUS[name]} ${name}`);
    }
  }
}

// ---- ② i18n.rs：MESSAGES 四元组 (code, "zh", "zhTW", "en") ----
// 四元组可能跨多行（rustfmt 折行）：按 `(` 分组抠配对括号内三个字符串。
const i18nSrc = read(I18N_RS);
const tableBody = i18nSrc.match(/const MESSAGES[^=]*=\s*&\[([\s\S]*?)\];/)?.[1]
  ?? "";
const i18n = new Map(); // code -> [zh, zhTW, en]
for (const entry of tableBody.matchAll(/\(\s*(\d+)\s*,/g)) {
  const start = entry.index + entry[0].length;
  let depth = 1, i = start;
  while (i < tableBody.length && depth > 0) {
    if (tableBody[i] === "(") depth++;
    else if (tableBody[i] === ")") depth--;
    i++;
  }
  const strs = [...tableBody.slice(start, i - 1)
    .matchAll(/"((?:[^"\\]|\\.)*)"/g)].map((m) => m[1]);
  if (strs.length >= 3) i18n.set(Number(entry[1]), strs.slice(0, 3));
}

// ---- ③ join：码 → 变体/触发条件/HTTP 态/三语文案 ----
let fails = 0;
const warns = [];
const rows = [];
for (const [variant, code] of variantCode) {
  const trig = codeTrigger.get(code);
  if (!trig) {
    console.error(
      `FAIL  ${code} ${variant}：code() 映射缺触发条件注释` +
      `（格式：// ${code} ${variant}：触发条件）`,
    );
    fails++;
  }
  const http = variantStatus.get(variant);
  if (!http) {
    console.error(`FAIL  ${variant}：status() 映射未解析到 HTTP 状态码`);
    fails++;
  }
  if (!i18n.has(code) && !KNOWN_GAPS.has(code)) {
    console.error(`FAIL  ${code}：i18n.rs MESSAGES 缺该码且不在 KNOWN_GAPS`);
    fails++;
  }
  rows.push({ variant, code, trig, http, msg: i18n.get(code) });
}
if (i18n.size === 0) {
  console.error("FAIL  i18n.rs MESSAGES 表一条都没抓到（解析器坏了？）");
  fails++;
}
rows.sort((a, b) => a.code - b.code || a.variant.localeCompare(b.variant));

// 变体名 → SCREAMING_SNAKE（CamelCase 词边界切分：TwoFactorRequired →
// TWO_FACTOR_REQUIRED；连续大写按词整体（HTTPError → HTTP_ERROR））
const snake = (s) =>
  s.replace(/([A-Z]+)([A-Z][a-z])/g, "$1_$2")
    .replace(/([a-z\d])([A-Z])/g, "$1_$2")
    .toUpperCase();

const esc = (s) => s.replace(/\|/g, "\\|");

// ---- ④ 生成 docs/api/error-codes.md ----
const gaps = rows.filter((r) => !r.msg);
const md = [];
md.push("# API 错误码注册表（生成物，勿手改）");
md.push("");
md.push("> 由 `node scripts/error_codes_gen.mjs --update` 生成（W-13）：");
md.push(`> ${ERRORS_RS}（变体→码→触发条件注释 + HTTP 态）、`);
md.push(`> ${I18N_RS}（三语文案）。CI 跑 \`--check\`：源码改动而未刷新 → 红灯。`);
md.push("> 码集对拍（与 index.ts 手写 ErrorCode）见 check_type_drift.mjs 第 4 维。");
md.push("");
md.push(
  `- 变体 ${variantCode.size} 个 → 错误码 ${new Set(variantCode.values()).size}` +
  ` 个；i18n 文案 ${i18n.size} 条（缺 ${gaps.length} 见文末待补清单）`,
);
md.push("");
md.push("| 码 | 变体 | 触发条件 | zh-CN | zh-TW | en | HTTP |");
md.push("|----:|------|----------|-------|-------|----|------|");
for (const r of rows) {
  md.push(
    `| ${r.code} | ${r.variant} | ${esc((r.trig ?? ["—"]).join("<br>"))} | ` +
    `${r.msg ? esc(r.msg[0]) : "—"} | ${r.msg ? esc(r.msg[1]) : "—"} | ` +
    `${r.msg ? esc(r.msg[2]) : "—"} | ${r.http ?? "—"} |`,
  );
}
if (gaps.length) {
  md.push("");
  md.push("## 待补文案（不阻塞，存量缺口同 error_registry.mjs KNOWN_GAPS）");
  md.push("");
  for (const r of gaps) {
    md.push(`- ${r.code}（${r.variant}）：MESSAGES 缺四元组，运行时回退 1000`);
  }
}
md.push("");
const mdDoc = md.join("\n");

// ---- ⑤ 生成 error-codes.generated.ts ----
// 不替换 index.ts 手写 ErrorCode（F4 消费端的事）；index.ts 末尾 re-export。
const ts = [];
ts.push("// 生成物，勿手改（node scripts/error_codes_gen.mjs --update）");
ts.push("// 源：apps/api/src/errors.rs 的 code() 映射（变体名 SCREAMING_SNAKE）");
ts.push("// 注意：同码多变体（1002 ← Validation/FieldErrors）后者覆盖前者；");
ts.push("// 前端消费端常量表仍是 index.ts 手写 ErrorCode（F4 替换，两处并存对拍）");
ts.push("export const ERROR_CODES = {");
for (const r of rows) {
  ts.push(`  ${snake(r.variant)}: ${r.code},`);
}
ts.push("} as const;");
ts.push("");
const tsDoc = ts.join("\n");

// ---- --check / --update ----
const argv = process.argv.slice(2);
const sameOrDie = (path_, stored, gen, label) => {
  if (!existsSync(path_)) {
    console.error(`ERROR_CODES_GEN_FAIL ${label} 不存在；先 --update 生成入库`);
    process.exit(1);
  }
  if (stored !== gen) {
    console.error(
      `ERROR_CODES_GEN_FAIL ${label} 过期（源码改了没刷新）` +
      `；修复：node scripts/error_codes_gen.mjs --update`,
    );
    process.exit(1);
  }
};
if (argv.includes("--check")) {
  sameOrDie(OUT_MD, readFileSync(OUT_MD, "utf8"), mdDoc, "error-codes.md");
  sameOrDie(
    OUT_TS, readFileSync(OUT_TS, "utf8"), tsDoc, "error-codes.generated.ts",
  );
  for (const w of warns) console.log("WARN ", w);
  console.log(
    `ERROR_CODES_GEN_OK ${rows.length} 行注册表` +
    `（${variantCode.size} 变体 / i18n ${i18n.size}）`,
  );
  process.exit(fails === 0 ? 0 : 1);
}

writeFileSync(OUT_MD, mdDoc);
writeFileSync(OUT_TS, tsDoc);
for (const w of warns) console.log("WARN ", w);
console.log(
  `已生成：error-codes.md ${rows.length} 行 + error-codes.generated.ts ` +
  `${variantCode.size} 常量（FAIL ${fails}）`,
);
process.exit(fails === 0 ? 0 : 1);
