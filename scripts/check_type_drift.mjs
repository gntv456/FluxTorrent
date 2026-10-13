/**
 * 契约漂移校验（六维强化方案 P0-7）：domain-types ↔ Rust 后端。
 *
 * 背景：packages/domain-types/src/index.ts:4 声称「CI openapi drift 校验」但该
 * 任务从未存在——后端加错误码/改枚举，前端静默不报错，线上才暴露。四维：
 *   1. 错误码：errors.rs `fn code()` ↔ TS ErrorCode（Rust 有 TS 无=FAIL，
 *      反向 WARN。注：全量对拍/文档生成见 error_registry / error_codes_gen）
 *   2. 促销枚举：promo.rs KIND_RANK ↔ PromotionKind（缺=FAIL）
 *   3. 列表响应形状：Rust Serialize 结构体 ↔ TS interface（TS 多声明=FAIL，
 *      **B1**：admin forums 的 categories 曾是元组序列化成数组的数组，
 *      前端按 c.id 取值恒 undefined，当时无任何门禁能发现）
 *   4. 生成物 ERROR_CODES ↔ 手写 ErrorCode（W-13：多/缺都 WARN 不阻断）
 *
 * 维护约定：新增/改动列表类接口的响应结构体时，在 CONTRACTS 表里加一行。
 * 找不到结构体/interface 会 FAIL（防止契约被悄悄删掉后门禁变成空转）。
 *
 * 用法：node scripts/check_type_drift.mjs（退出码 0=通过 1=漂移）。
 */
import { readFileSync } from "node:fs";
import { fileURLToPath } from "node:url";
import path from "node:path";

const root = path.resolve(path.dirname(fileURLToPath(import.meta.url)), "..");
const read = (p) => readFileSync(path.join(root, p), "utf8");
// 名→码表：TS/生成物常量块的通用抓法（`export const X = {...} as const`）
const parseConstMap = (src, constName) => {
  const body = src.match(
    new RegExp(`export const ${constName} = \\{([\\s\\S]*?)\\} as const`),
  )?.[1] ?? "";
  const m = new Map();
  for (const c of body.matchAll(/(\w+)\s*:\s*(\d+)/g)) {
    m.set(c[1], Number(c[2]));
  }
  return m;
};

let fails = 0;
const warns = [];
const section = (t) => console.log(`\n== ${t} ==`);

// ---------- 1. 错误码 ----------
section("错误码 errors.rs ↔ ErrorCode");
const rustCodes = new Map();
for (const m of read("apps/api/src/errors.rs").matchAll(
  /DomainError::(\w+)(?:\([^)]*\))?\s*=>\s*(\d+)/g,
)) {
  rustCodes.set(m[1], Number(m[2]));
}
const tsCodes = parseConstMap(
  read("packages/domain-types/src/index.ts"), "ErrorCode",
);
console.log(`rust=${rustCodes.size} 个变体, ts=${tsCodes.size} 个常量`);

// 反向索引：TS 值 → 名称（同一数值改名不算漂移）
const tsByValue = new Map([...tsCodes].map(([k, v]) => [v, k]));
for (const [variant, code] of rustCodes) {
  if (!tsByValue.has(code)) {
    console.log(`FAIL  Rust ${variant}=${code} 在 TS ErrorCode 中不存在`);
    fails++;
  }
}
const rustValues = new Set(rustCodes.values());
for (const [name, code] of tsCodes) {
  if (!rustValues.has(code)) {
    warns.push(
      `TS ErrorCode.${name}=${code} 未在 errors.rs 的 code() 出现（预留码？）`,
    );
  }
}

// ---------- 2. 促销枚举 ----------
section("促销枚举 promo.rs KIND_RANK ↔ PromotionKind");
// 只取 KIND_RANK 常量字符串内的 WHEN 'kind' 字面量——matched() 里的
// 'global'/'official'/'category' 是 scope 不是 kind，混进来会误报。
const kindRank =
  read("apps/api/src/torrents/promo.rs").match(
    /const KIND_RANK[\s\S]*?= "([\s\S]*?)";/,
  )?.[1] ?? "";
const rustKinds = new Set(
  [...kindRank.matchAll(/WHEN '([a-z0-9]+)'/g)].map((m) => m[1]),
);
const tsPromoBlock = read("packages/domain-types/src/index.ts").match(
  /export type PromotionKind =([\s\S]*?);/,
)?.[1] ?? "";
const tsKinds = new Set(
  [...tsPromoBlock.matchAll(/"([a-z0-9]+)"/g)].map((m) => m[1]),
);
console.log(`rust=${[...rustKinds].join(",")} | ts=${[...tsKinds].join(",")}`);
for (const k of rustKinds) {
  if (!tsKinds.has(k)) {
    console.log(`FAIL  促销 kind '${k}' 出现在后端优先级表但不在 TS PromotionKind`);
    fails++;
  }
}

// ---------- 3. 列表响应形状 ----------
section("响应形状 Rust struct ↔ TS interface");

/** 从 Rust/TS 源码里按括号配对抠出 `open` 之后到配对闭括号的函数体。
 *  rustStruct / tsInterface 共用的取体原语。 */
function braceBody(src, openIdx) {
  let i = openIdx;
  let depth = 0;
  const start = i;
  while (i < src.length) {
    const ch = src[i];
    if (ch === "{") depth++;
    else if (ch === "}") {
      depth--;
      if (depth === 0) return src.slice(start + 1, i);
    }
    i++;
  }
  return "";
}

/** 从 Rust 源码里抠出 `struct Name { ... }` 的字段名集合。
 *
 *  只认「行首的 `name:`」，属性行（`#[...]`）与注释行跳过。
 *  本仓的响应结构体都是扁平数据类，字段类型不跨行，够用。
 *  返回 null 表示找不到定义。 */
function rustStruct(src, name) {
  const m = new RegExp(
    `(?:^|\\n)(?:pub(?:\\([^)]*\\))?\\s+)?struct\\s+${name}\\s*\\{`,
  ).exec(src);
  if (!m) return null;
  const body = braceBody(src, m.index + m[0].length - 1);
  // serde rename 会改真实 JSON 键名，静态对不出来 —— 标记出来让人工确认
  const head = src.slice(Math.max(0, m.index - 300), m.index);
  const renamed = /#\[serde\([^)]*rename/.test(head);
  const fields = new Set();
  for (const line of body.split("\n")) {
    const t = line.trim();
    if (!t || t.startsWith("//") || t.startsWith("#")) continue;
    const fm = /^(?:pub(?:\s*\([^)]*\))?\s+)?([a-z_]\w*)\s*:/.exec(t);
    if (fm) fields.add(fm[1]);
  }
  return { fields, renamed };
}

/** 从 TS 源码里抠出 `export interface Name { ... }` 的字段名集合。
 *  可选字段（`foo?:`）算作存在。返回 null 表示找不到定义。 */
function tsInterface(src, name) {
  const m = new RegExp(`export\\s+interface\\s+${name}\\s*\\{`).exec(src);
  if (!m) return null;
  const body = braceBody(src, m.index + m[0].length - 1);
  const fields = new Set();
  for (const line of body.split("\n")) {
    const t = line.trim();
    if (!t || t.startsWith("//") || t.startsWith("/*") || t.startsWith("*")) {
      continue;
    }
    const fm = /^([A-Za-z_]\w*)\s*\??\s*:/.exec(t);
    if (fm) fields.add(fm[1]);
  }
  return { fields };
}

/** 契约断言表。`strict: true` 表示「Rust 字段必须全部在 TS 里」（双向镜像）。
 *  没标 strict 的只当 WARN —— 前端有意只用子集时不该挂 CI。
 *  紧凑行式（label | rust 文件 | struct | ts 文件 | interface | strict）：
 *  8 条逐字段写满 72 行，同样的信息一行放下，给上面解析器留预算。 */
const DT = "packages/domain-types/src/index.ts";
const ADMIN_TYPES =
  "apps/web/app/(main)/admin/forums/_parts/forum-structure-types.ts";
const CONTRACTS = [
  ["版块列表 GET /forums", "community_http/forums.rs", "ForumRow", DT, "Forum"],
  ["分区（前台）GET /forums.categories", "community_http/forums.rs",
    "ForumCatRow", DT, "ForumCategory"],
  ["分区（后台）GET /admin/forums.categories", "admin_http/forum.rs",
    "ForumCategoryRow", DT, "ForumCategory"],
  ["版块（后台）GET /admin/forums.forums", "admin_http/forum.rs",
    "ForumAdminRow", ADMIN_TYPES, "ForumAdminForum"],
  ["版块精简 GET /forums/boards", "community_http/forums.rs",
    "BoardBrief", DT, "BoardBrief"],
  // 农场两投影（0253 作物行表 / 0260 地块阶梯）：两侧字段必须逐字对齐，
  // 接口改了前端没跟就是「读到 undefined」。
  ["农场地块 GET /farm.plots", "games_http/farm.rs", "PlotRow",
    "apps/web/components/game/farm-field.tsx", "Plot"],
  ["农场作物行情 GET /farm.crops", "games_http/farm.rs", "CropRow",
    "apps/web/components/game/farm-art.tsx", "Crop"],
  // 奖池档位（/games 五玩法共用投影）：原 json! 动态拼、门禁盖不到——
  // 「TS 声明了、Rust 没返回」正是页面静默空白的来源（B1 型补盲区）。
  ["奖池档位 GET /games.prizes", "games_http/prize_view.rs", "PrizeRow",
    "apps/web/lib/games.ts", "JggPrizeView"],
].map(([label, rust, struct, ts, iface]) => ({
  label,
  rust: `apps/api/src/${rust}`,
  struct,
  ts,
  iface,
  strict: true,
}));

const cache = new Map();
const src = (p) => {
  if (!cache.has(p)) cache.set(p, read(p));
  return cache.get(p);
};

for (const c of CONTRACTS) {
  const r = rustStruct(src(c.rust), c.struct);
  const t = tsInterface(src(c.ts), c.iface);
  if (!r || !t) {
    const miss = [
      !r ? `Rust ${c.struct}` : null,
      !t ? `TS ${c.iface}` : null,
    ].filter(Boolean).join(" + ");
    console.log(`FAIL  ${c.label}：找不到 ${miss}（契约被删/改名？同步更新本表）`);
    fails++;
    continue;
  }
  const onlyTs = [...t.fields].filter((f) => !r.fields.has(f));
  const onlyRust = [...r.fields].filter((f) => !t.fields.has(f));
  const note = r.renamed ? "  ⚠️ 该结构体带 serde rename，键名请人工核对" : "";
  console.log(
    `  ${c.label}：rust ${r.fields.size} 字段 / ts ${t.fields.size} 字段${note}`,
  );
  for (const f of onlyTs) {
    console.log(
      `FAIL  ${c.label}：TS 声明了 \`${f}\` 但 Rust ${c.struct} 不返回` +
        ` → 前端会读到 undefined（B1 型）`,
    );
    fails++;
  }
  for (const f of onlyRust) {
    const msg = `${c.label}：Rust ${c.struct} 多了 \`${f}\`，TS ${c.iface} 未声明`;
    if (c.strict) {
      console.log(`FAIL  ${msg}（本契约要求双向镜像）`);
      fails++;
    } else {
      warns.push(`${msg}（前端只用子集则无需处理）`);
    }
  }
}

// ---------- 4. 生成物常量 ↔ 手写 ErrorCode 对拍（W-13 注册表） ----------
section("错误码生成物 ERROR_CODES ↔ 手写 ErrorCode");
// error-codes.generated.ts 按 errors.rs code() 生成；index.ts 的 ErrorCode
// 是手写消费端常量（F4 才替换）。本步是揭示不是阻断：多/缺都只 WARN——
// TS 多出的值 = 预留码（1001/2008/3001/3002/5001，后端未发出），生成物
// 多出的值 = 新码已进后端但手写常量未跟（消费端该补）。
const genCodes = parseConstMap(
  read("packages/domain-types/src/error-codes.generated.ts"),
  "ERROR_CODES",
);
console.log(`generated=${genCodes.size} 常量, 手写=${tsCodes.size} 常量`);
// 按值对拍（同名异值/同值异名都归到值集差异，避免两套命名的假漂移）
const tsVals = new Set(tsCodes.values());
const genVals = new Set(genCodes.values());
for (const [name, code] of tsCodes) {
  if (!genVals.has(code)) {
    warns.push(
      `TS ErrorCode.${name}=${code} 不在生成物 ERROR_CODES 中` +
        `（预留码：后端 code() 未发出，F4 替换时需并入生成物或标注保留）`,
    );
  }
}
for (const [name, code] of genCodes) {
  if (!tsVals.has(code)) {
    warns.push(
      `生成物 ERROR_CODES.${name}=${code} 不在手写 ErrorCode 中` +
        `（后端已有该码，消费端常量未跟——补 index.ts 或确认为弃用码）`,
    );
  }
}
if (genCodes.size === 0) {
  // 生成物被清空/损坏不是「漂移」是事故：直接挂
  console.log("FAIL  error-codes.generated.ts 一条常量都没抓到（文件损坏？）");
  fails++;
}

// ---------- 汇总 ----------
if (warns.length) {
  console.log("\n-- WARN --");
  for (const w of warns) console.log("WARN ", w);
}
console.log(
  fails === 0
    ? "\nTYPE_DRIFT_OK 契约一致"
    : `\nTYPE_DRIFT_FAIL ${fails} 处漂移`,
);
process.exit(fails === 0 ? 0 : 1);
