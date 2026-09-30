/**
 * 契约漂移校验（六维强化方案 P0-7）：domain-types ↔ Rust 后端。
 *
 * 背景：packages/domain-types/src/index.ts:4 声称「CI openapi drift 校验」但该任务从未存在——
 * 后端加错误码/改枚举，前端静默不报错，线上才暴露。本脚本做最小可靠对比：
 *   1. 错误码：errors.rs 的 `fn code()` 映射 ↔ TS ErrorCode 常量表
 *      - Rust 有而 TS 无 → FAIL（前端无法识别该错误码）
 *      - TS 有而 Rust 无 → WARN（预留或 json! 手写码，人工确认）
 *   2. 促销枚举：torrents/promo.rs 的 KIND_RANK 字面量 ↔ TS PromotionKind
 *      - 促销优先级表里出现而 TS 枚举没有 → FAIL（徽标/筛选会漂移）
 *   3. 列表响应形状：Rust 的 `#[derive(Serialize)]` 结构体字段 ↔ TS interface
 *      - TS 声明了而 Rust 结构体没有 → FAIL（前端会读到 undefined，**B1 就是这么来的**）
 *      - Rust 有而 TS 没声明 → WARN（多为前端只用子集，但新增字段时应跟上）
 *
 * 为什么单列第 3 维（2026-09-22）：`GET /admin/forums` 的 categories 曾是
 * `Vec<(i64,String,i32,bool)>`，serde 序列化成**数组的数组**，而前端按 `c.id`/`c.name`
 * 取字段 → 恒 undefined、分区名全空白。当时**没有任何门禁能发现**：类型检查看不出、
 * 运行时也不报错。修完必须留个机械拦网，否则下次照样有人写成元组。
 *
 * 维护约定：**新增/改动列表类接口的响应结构体时，在 CONTRACTS 表里加一行**。
 * 找不到结构体/interface 会 FAIL（防止契约被悄悄删掉后门禁变成空转）。
 *
 * 用法：node scripts/check_type_drift.mjs   （退出码 0=通过 1=漂移）
 * CI 挂载：build-test.yml 的 web 任务里加一步
 *   `node scripts/check_type_drift.mjs`（在 pnpm lint 之前）。
 */
import { readFileSync } from "node:fs";
import { fileURLToPath } from "node:url";
import path from "node:path";

const root = path.resolve(path.dirname(fileURLToPath(import.meta.url)), "..");
const read = (p) => readFileSync(path.join(root, p), "utf8");

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
const tsBlock =
  read("packages/domain-types/src/index.ts").match(
    /export const ErrorCode = \{([\s\S]*?)\} as const/,
  )?.[1] ?? "";
const tsCodes = new Map();
for (const m of tsBlock.matchAll(/(\w+)\s*:\s*(\d+)/g)) {
  tsCodes.set(m[1], Number(m[2]));
}
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
  let i = m.index + m[0].length;
  let depth = 1;
  let body = "";
  while (i < src.length && depth > 0) {
    const ch = src[i];
    if (ch === "{") depth++;
    else if (ch === "}") {
      depth--;
      if (depth === 0) break;
    }
    body += ch;
    i++;
  }
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
  let i = m.index + m[0].length;
  let depth = 1;
  let body = "";
  while (i < src.length && depth > 0) {
    const ch = src[i];
    if (ch === "{") depth++;
    else if (ch === "}") {
      depth--;
      if (depth === 0) break;
    }
    body += ch;
    i++;
  }
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

const DT = "packages/domain-types/src/index.ts";
const ADMIN_TYPES =
  "apps/web/app/(main)/admin/forums/_parts/forum-structure-types.ts";

/** 契约断言表。`strict: true` 表示「Rust 字段必须全部在 TS 里」（双向镜像）。
 *  没标 strict 的只当 WARN —— 前端有意只用子集时不该挂 CI。 */
const CONTRACTS = [
  {
    label: "版块列表 GET /forums",
    rust: "apps/api/src/community_http/forums.rs",
    struct: "ForumRow",
    ts: DT,
    iface: "Forum",
    strict: true,
  },
  {
    label: "分区（前台）GET /forums.categories",
    rust: "apps/api/src/community_http/forums.rs",
    struct: "ForumCatRow",
    ts: DT,
    iface: "ForumCategory",
    strict: true,
  },
  {
    label: "分区（后台）GET /admin/forums.categories",
    rust: "apps/api/src/admin_http/forum.rs",
    struct: "ForumCategoryRow",
    ts: DT,
    iface: "ForumCategory",
    strict: true,
  },
  {
    label: "版块（后台）GET /admin/forums.forums",
    rust: "apps/api/src/admin_http/forum.rs",
    struct: "ForumAdminRow",
    ts: ADMIN_TYPES,
    iface: "ForumAdminForum",
    strict: true,
  },
  {
    label: "版块精简 GET /forums/boards",
    rust: "apps/api/src/community_http/forums.rs",
    struct: "BoardBrief",
    ts: DT,
    iface: "BoardBrief",
    strict: true,
  },
  // 娱乐屋 · 农场：`GET /farm` 的两行表投影。作物表自 0253 起是站长可配的行表
  // （`active` = 下架位），地块投影自 0260 起与土地阶梯同页读 —— 两侧字段
  // 必须逐字对齐，接口改了前端没跟就是「读到 undefined」。
  {
    label: "农场地块 GET /farm.plots",
    rust: "apps/api/src/games_http/farm.rs",
    struct: "PlotRow",
    ts: "apps/web/components/game/farm-field.tsx",
    iface: "Plot",
    strict: true,
  },
  {
    label: "农场作物行情 GET /farm.crops",
    rust: "apps/api/src/games_http/farm.rs",
    struct: "CropRow",
    ts: "apps/web/components/game/farm-art.tsx",
    iface: "Crop",
    strict: true,
  },
  // 奖池档位（/games 各池的 prizes 行）：五玩法共用一份投影。
  // 这一行是**补盲区** —— 该投影原先用 json! 动态拼，门禁盖不到，
  // 而「TS 声明了、Rust 没返回」正是页面静默空白的来源（B1 型）。
  {
    label: "奖池档位 GET /games.prizes",
    rust: "apps/api/src/games_http/prize_view.rs",
    struct: "PrizeRow",
    ts: "apps/web/lib/games.ts",
    iface: "JggPrizeView",
    strict: true,
  },
];

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
