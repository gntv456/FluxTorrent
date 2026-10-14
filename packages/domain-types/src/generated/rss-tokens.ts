/**
 * rss-tokens 域接口类型（自动生成，勿手改）。
 * 生成器：scripts/gen_ts_types.mjs（FRAME §7 第 8 条分域起步）
 * 来源：/api/v1/openapi.json 的 #[utoipa::path] 注解聚合。
 * 再生成：node scripts/gen_ts_types.mjs
 * 覆盖端点：
 *   GET /api/v1/rss/tokens — 列出我的有效 RSS token 元数据（不回明文，库里只有哈希）。
 *   POST /api/v1/rss/tokens — POST /rss/tokens：创建命名 token；明文仅本次响应可见（no_store
 *   DELETE /api/v1/rss/tokens/{id} — 软删 token（revoked_at=now()，历史行留审计）。
 *   POST /api/v1/rss/tokens/{id}/rotate — 轮换：吊销旧 token + 新建同名 token（明文仅本次响应可
 */

/** /rss/tokens 列表项元数据（无明文——库里只有哈希） */
export interface RssTokenMeta {
  id: number;
  name: string;
  created_at: string;
  last_used_at: string | null;
}

/** 创建/轮换响应：元数据 + 一次性明文 token */
export interface RssTokenCreated extends RssTokenMeta {
  token: string;
  hint: string;
}

/** 创建请求体 */
export interface RssTokenCreateReq {
  name: string;
}

/** GET /rss/tokens 响应体 */
export interface RssTokenList {
  tokens: RssTokenMeta[];
  limit: number;
}
