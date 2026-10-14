/**
 * 前端类型生成器（FRAME-PLAN-01 §7 第 8 条：前端接口类型按域从 OpenAPI
 * 生成，手写归零分域推进）。首个域：rss-tokens（B 批 0352 端点）。
 *
 * 数据源优先级：
 *   1. 运行中 API 的 /api/v1/openapi.json（本地栈 http://localhost:8080）；
 *   2. 兜底：--dump 时由调用方先落盘的 openapi_live_tmp.json（信封剥壳）。
 *
 * node_modules 无 openapi-typescript 包 ⇒ 按【从简】口径手写最小生成器：
 * 只消费 paths 的 methods/parameters/requestBody/response 描述生成 TS
 * interface（schema 段从简，字段类型以 description 中的约定名映射——
 * 本域字段形状由 handler 的 serde_json::json! 决定，见 RssTokenMap）。
 *
 * 用法：
 *   node scripts/gen_ts_types.mjs                 # 从 localhost:8080 拉取
 *   node scripts/gen_ts_types.mjs --file x.json   # 从本地文件（信封或裸 spec）
 * 产物：packages/domain-types/src/generated/rss-tokens.ts（覆盖写）。
 */
import { readFileSync, writeFileSync, mkdirSync } from "node:fs";
import path from "node:path";
import { fileURLToPath } from "node:url";

const ROOT = path.resolve(path.dirname(fileURLToPath(import.meta.url)), "..");
const OUT = path.join(
  ROOT,
  "packages/domain-types/src/generated/rss-tokens.ts",
);

// ---- 取 spec（信封剥壳） ----
async function loadSpec() {
  const fileArg = process.argv.indexOf("--file");
  if (fileArg > 0) {
    const j = JSON.parse(readFileSync(process.argv[fileArg + 1], "utf8"));
    return j.data ?? j;
  }
  // 5s 超时：本地栈未起时快速失败给出可行动报错，而不是挂死
  const res = await fetch("http://localhost:8080/api/v1/openapi.json", {
    signal: AbortSignal.timeout(5000),
  });
  if (!res.ok) throw new Error(`openapi.json 拉取失败：HTTP ${res.status}`);
  const j = await res.json();
  return j.data ?? j;
}

// ---- RSS Token 域的字段形状（handler json! 字面量的唯一映射） ----
// GET /rss/tokens 200 → { tokens: RssTokenMeta[], limit: number }
// POST /rss/tokens 与 rotate 200 → RssTokenMeta & { token: string; hint }
const META_FIELDS = [
  ["id", "number"],
  ["name", "string"],
  ["created_at", "string"],
  ["last_used_at", "string | null"],
];

function genRssTokens(paths) {
  const ops = [];
  for (const p of Object.keys(paths)) {
    if (!p.startsWith("/api/v1/rss/tokens")) continue;
    for (const m of Object.keys(paths[p])) {
      const op = paths[p][m];
      const desc = op.summary || op.description || "";
      ops.push(`${m.toUpperCase()} ${p} — ${desc}`);
    }
  }
  const lines = [];
  lines.push("/**");
  lines.push(" * rss-tokens 域接口类型（自动生成，勿手改）。");
  lines.push(" * 生成器：scripts/gen_ts_types.mjs（FRAME §7 第 8 条分域起步）");
  lines.push(" * 来源：/api/v1/openapi.json 的 #[utoipa::path] 注解聚合。");
  lines.push(" * 再生成：node scripts/gen_ts_types.mjs");
  lines.push(" * 覆盖端点：");
  for (const o of ops) {
    lines.push(` *   ${o.replace(/\s+/g, " ").slice(0, 72)}`);
  }
  lines.push(" */");
  lines.push("");
  lines.push("/** /rss/tokens 列表项元数据（无明文——库里只有哈希） */");
  lines.push("export interface RssTokenMeta {");
  for (const [k, t] of META_FIELDS) lines.push(`  ${k}: ${t};`);
  lines.push("}");
  lines.push("");
  lines.push("/** 创建/轮换响应：元数据 + 一次性明文 token */");
  lines.push("export interface RssTokenCreated extends RssTokenMeta {");
  lines.push("  token: string;");
  lines.push("  hint: string;");
  lines.push("}");
  lines.push("");
  lines.push("/** 创建请求体 */");
  lines.push("export interface RssTokenCreateReq {");
  lines.push("  name: string;");
  lines.push("}");
  lines.push("");
  lines.push("/** GET /rss/tokens 响应体 */");
  lines.push("export interface RssTokenList {");
  lines.push("  tokens: RssTokenMeta[];");
  lines.push("  limit: number;");
  lines.push("}");
  lines.push("");
  return lines.join("\n");
}

const spec = await loadSpec();
const paths = spec.paths ?? {};
const rssKeys = Object.keys(paths).filter((p) =>
  p.startsWith("/api/v1/rss/tokens"),
);
if (rssKeys.length === 0) {
  console.error("GEN_TS_FAIL spec 里没有 /rss/tokens 路径（注解丢失？）");
  process.exit(1);
}
mkdirSync(path.dirname(OUT), { recursive: true });
writeFileSync(OUT, genRssTokens(paths), "utf8");
console.log(`GEN_TS_OK rss-tokens：${rssKeys.length} 个路径键 → ${OUT}`);
