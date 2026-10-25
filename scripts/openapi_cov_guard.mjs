/**
 * OpenAPI 覆盖门禁（FRAME-PLAN-01 F3 / W-49）：注解路径 ⊇ 手写域全集。
 *
 * 背景：/api/v1/openapi.json 的路径表自 W-49 起由 handler 上的
 * `#[utoipa::path(...)]` 聚合生成（open_api.rs + compat_api.rs），
 * 手写路径段（原 spec.rs paths_* / spec_np.rs）已删除。注解是新的单源，
 * 但「注解被误删/路径被改」没有任何静态拦截——本门补上：
 *
 *   维度 1（注解面）：compat + 开放两域源文件里 `#[utoipa::path(` 的数量
 *     与 path = "/api/v1/..." 行数一致（方法宏丢 path 即红）；
 *   维度 2（产物面）：openapi_cov_guard 无法直接调 Rust 聚合（需编译运行），
 *     改为对拍「route-manifest 中的两域路由」与「源码注解 path」：
 *     注解 path 集合必须 ⊇ 手写域 28 条路由（route-manifest 是 W-12
 *     防漂移清单，路由宏侧已被它锁定，此处只锁 spec 侧不丢）。
 *
 * 基线见同目录 openapi_cov_baseline.json；本门与 cargo test 里的
 * openapi_json_covers_all_annotated_domains 互为冗余（测试锁产物、
 * 本门锁源码形态，CI 不跑 cargo 时也有兜底）。
 *
 * 用法：node scripts/openapi_cov_guard.mjs（退出码 0=通过 1=缺口）。
 * 红测：临时删掉任一 handler 的 #[utoipa::path] 块 → 维度 2 报缺口 exit 1。
 */
import { readFileSync, writeFileSync } from "node:fs";
import { fileURLToPath } from "node:url";
import path from "node:path";

const root = path.resolve(path.dirname(fileURLToPath(import.meta.url)), "..");
const read = (p) => readFileSync(path.join(root, p), "utf8");
import {
  ANNOTATED_FILES,
  checkCommunityCoverage,
} from "./openapi_cov_domain.mjs";

let fails = 0;
const section = (t) => console.log(`\n== ${t} ==`);

// 两域注解所在的源文件（W-49 清单；新增域文件时在这里加行）
// 手写域全集（W-49 迁移时的 27 条 + 注解化后补齐的 /downloads/keys/revoke）
// —— route-manifest 口径：scope 相对路径（不带 /api/v1 前缀）
const DOMAIN_ROUTES = [
  // 开放 API 域（14 操作 / 13 键）
  ["GET", "/open/recent"],
  ["GET", "/open/announces"],
  ["GET", "/open/categories"],
  ["POST", "/open/torrents"],
  ["GET", "/open/torrents/{id}"],
  ["GET", "/open/me/overview"],
  ["GET", "/open/me/hr"],
  ["GET", "/open/me/messages"],
  ["GET", "/open/me/seeding"],
  ["GET", "/open/me/history"],
  ["GET", "/me/tokens"],
  ["POST", "/me/tokens"],
  ["POST", "/me/tokens/revoke"],
  ["POST", "/me/tokens/refresh"],
  // compat / Torznab / RSS 域（15 操作；revoke 为注解化补齐）
  ["GET", "/compat/nexusphp/user.json"],
  ["GET", "/compat/nexusphp/torrents.json"],
  ["GET", "/compat/nexusphp/torrent/{id}.json"],
  ["GET", "/compat/nexusphp/download.php"],
  ["GET", "/compat/nexusphp/getrss.php"],
  ["POST", "/compat/nexusphp/takelogin.php"],
  ["GET", "/compat/nexusphp/userdetails.php"],
  ["GET", "/compat/nexusphp/details.php"],
  ["GET", "/plugins/ptppUserInfo"],
  ["POST", "/downloads/keys"],
  ["POST", "/downloads/keys/revoke"],
  ["GET", "/downloads/{torrent_id}"],
  ["GET", "/torznab"],
  ["GET", "/torznab/search"],
  ["GET", "/rss/{passkey}"],
];

// ---------- 1. 注解面形态：#[utoipa::path( 与 path = 数量一致 ----------
section("注解形态：utoipa::path 块数 ↔ path= 行数");
let blocks = 0;
let pathLines = 0;
for (const f of ANNOTATED_FILES) {
  const src = read(f);
  const b = [...src.matchAll(/#\[utoipa::path\(/g)].length;
  const p = [...src.matchAll(/path\s*=\s*"\/api\/v1\//g)].length;
  blocks += b;
  pathLines += p;
  if (b !== p) {
    console.log(`FAIL  ${f}：${b} 个 #[utoipa::path] 块但 ${p} 行 path=（丢 path 属性？）`);
    fails++;
  }
}
console.log(`annotated files=${ANNOTATED_FILES.length}, blocks=${blocks}, path lines=${pathLines}`);
if (blocks === 0) {
  console.log("FAIL  一个 #[utoipa::path] 都没抓到（文件清单失效？）");
  fails++;
}

// ---------- 2. 产物面：注解 path ⊇ 手写域路由全集 ----------
section("注解 path 集合 ⊇ 手写域 28 条路由");
const annotated = new Set();
for (const f of ANNOTATED_FILES) {
  for (const m of read(f).matchAll(
    /#\[(?:get|post|put|delete|patch)\("([^"]+)"\)\]/g,
  )) {
    // route 宏还在（路由侧由 route_manifest_guard 单独把守），这里只收集
  }
}
// 直接从注解 path 提取（带 /api/v1 前缀，转为 manifest 口径）
for (const f of ANNOTATED_FILES) {
  for (const m of read(f).matchAll(/path\s*=\s*"\/api\/v1(\/[^"]*)"/g)) {
    annotated.add(m[1]);
  }
}
// path 级对拍（方法级由 route-manifest + cargo 单测双层锁定，本门锁 path 不丢）
const missing = [...new Set(DOMAIN_ROUTES.map(([, p]) => p))].filter(
  (p) => !annotated.has(p),
);
console.log(`annotated paths=${annotated.size}, domain path keys=${new Set(DOMAIN_ROUTES.map(([, p]) => p)).size}`);
if (missing.length) {
  for (const p of missing) {
    console.log(`FAIL  手写域路由 ${p} 不在任何 #[utoipa::path] 注解里（spec 漂移）`);
  }
  fails++;
}

// ---------- 3. route-manifest 对拍（W-12 清单是路由侧事实源） ----------
section("route-manifest 对拍（域内 28 条路由必须在清单中）");
const manifest = JSON.parse(read("docs/api/route-manifest.json"));
const mset = new Set(manifest.routes.map((r) => `${r.method} ${r.path}`));
for (const [method, p] of DOMAIN_ROUTES) {
  if (!mset.has(`${method} ${p}`)) {
    console.log(`WARN  ${method} ${p} 不在 route-manifest（口径差异？人工核对）`);
  }
}


// ---------- 3.5 社区域对拍（F4：社区 92 条必须全部注解化） ----------
section("社区域 92 条路由 ↔ 注解对拍");
fails += checkCommunityCoverage(read, manifest);

// ---------- 4. 生成物面：gen_ts_types 落盘且与注解域对得上 ----------
// （FRAME §7 第 8 条分域起步：rss-tokens 域的 TS 类型来自 OpenAPI 生成，
//   手写接口已在 rss-tokens.tsx 删除——本步锁「生成物在且含关键键」，
//   防止 generated/ 被误删后前端悄悄回落到手写。）
section("生成物：packages/domain-types/src/generated/rss-tokens.ts");
try {
  const gen = read(
    "packages/domain-types/src/generated/rss-tokens.ts",
  );
  const need = [
    "RssTokenMeta",
    "RssTokenCreated",
    "RssTokenCreateReq",
    "RssTokenList",
  ];
  const miss = need.filter((k) => !gen.includes(`interface ${k}`));
  if (miss.length) {
    console.log(
      `FAIL  生成物缺关键类型 ${miss.join(", ")}（重跑 node scripts/gen_ts_types.mjs）`,
    );
    fails++;
  } else {
    console.log("OK  rss-tokens 生成类型 4/4 在位");
  }
  // 注解侧对拍：rss/tokens 路径键必须在源注解里（防止生成物来自旧 spec）
  const rssAnnotated = [
    ...read("apps/api/src/rss_http/tokens.rs").matchAll(
      /path\s*=\s*"\/api\/v1\/rss\/tokens[^"]*"/g,
    ),
  ].length;
  if (rssAnnotated < 3) {
    console.log(
      `FAIL  rss_http/tokens.rs 注解路径数 ${rssAnnotated} < 3（注解丢失？）`,
    );
    fails++;
  } else {
    console.log(`OK  rss-tokens 注解路径 ${rssAnnotated} 条`);
  }
} catch {
  console.log(
    "FAIL  packages/domain-types/src/generated/rss-tokens.ts 不存在（node scripts/gen_ts_types.mjs）",
  );
  fails++;
}

// ---------- 汇总 ----------
console.log(
  fails === 0
    ? "\nOPENAPI_COV_OK 注解覆盖完整"
    : `\nOPENAPI_COV_FAIL ${fails} 处缺口`,
);
process.exit(fails === 0 ? 0 : 1);
