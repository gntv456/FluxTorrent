/** openapi.json 的取数与分组（从 app/(main)/api-docs/page.tsx 抽出成纯函数）。
 *
 *  为什么单列一个文件：这个页面此前读的是 `json.paths`，而后站所有端点都套
 *  `ok()` 信封（`{code,message,data,request_id}`），于是 `json.paths` 恒
 *  undefined ⇒ 现网 163 个 operation 被算成 0 个，整页只剩一句「API 暂不可达」。
 *  这类「信封漏剥」的错，挂在 RSC 页面里没法用便宜的办法钉住（页面还要 next/headers
 *  取字典），抽成纯函数后一条用例就能同时钉住「带信封」「不带信封」「非 HTTP
 *  动词的键不算端点」三种输入。 */

export interface Endpoint {
  path: string;
  method: string;
  summary: string;
}

export type RawOp = { summary?: string; tags?: string[] };
export type RawSpec = {
  info?: { title?: string; version?: string };
  paths?: Record<string, Record<string, RawOp>>;
};

export const VERBS = ["get", "post", "put", "patch", "delete"];

/** 从响应体里取出规范本体：优先 `data`（站内统一 ok() 信封），
 *  没有信封的部署（直出 spec）也照单全收。 */
export function unwrapSpec(body: unknown): RawSpec {
  if (!body || typeof body !== "object") return {};
  const wrapped = (body as { data?: unknown }).data;
  const spec = wrapped && typeof wrapped === "object" ? wrapped : body;
  return (spec ?? {}) as RawSpec;
}

/** paths → 按 tag 分组的端点表。分组名取每个 operation 自己的 `tags[0]`
 *  （utoipa 组装时顶层 tags 数组被丢弃，只剩这一处可用）。 */
export function groupEndpoints(
  spec: RawSpec,
  ungrouped: string,
): { groups: { tag: string; items: Endpoint[] }[]; total: number } {
  const buckets = new Map<string, Endpoint[]>();
  let total = 0;
  for (const [path, ops] of Object.entries(spec.paths ?? {})) {
    for (const [verb, op] of Object.entries(ops ?? {})) {
      if (!VERBS.includes(verb.toLowerCase()) || typeof op !== "object") {
        continue;
      }
      const tag = op.tags?.[0] || ungrouped;
      const arr = buckets.get(tag) ?? [];
      arr.push({ path, method: verb.toUpperCase(), summary: op.summary ?? "" });
      buckets.set(tag, arr);
      total += 1;
    }
  }
  const groups = [...buckets.entries()]
    .map(([tag, items]) => ({
      tag,
      items: items.sort((a, b) => a.path.localeCompare(b.path)),
    }))
    .sort(
      (a, b) =>
        b.items.length - a.items.length || a.tag.localeCompare(b.tag),
    );
  return { groups, total };
}
