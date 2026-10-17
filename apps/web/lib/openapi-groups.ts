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
  /** 「展开」那一栏的内容：参数与已声明的响应码 */
  detail: OpDetail;
}

export type RawParam = {
  name?: string;
  in?: string;
  required?: boolean;
  schema?: { type?: string };
};
export type RawOp = {
  summary?: string;
  tags?: string[];
  parameters?: RawParam[];
  /** path 级参数（OpenAPI 允许挂在 path item 上，对本 path 下所有方法生效） */
  pathParameters?: RawParam[];
  responses?: Record<string, { description?: string }>;
};
export type RawSpec = {
  info?: { title?: string; version?: string };
  paths?: Record<string, Record<string, RawOp>>;
};

/** 一个端点的详情（样图 § api-docs 每行那个「展开」）：参数与已声明的响应码。
 *  只报规范里真写了的东西——没有 `responses` 就回空数组，界面写「未声明」，
 *  不拿 200 兜底假装规范里有。 */
export interface OpDetail {
  params: { name: string; loc: string; required: boolean; type: string }[];
  codes: string[];
}

/** 少数构建把动词键误写进 `responses`，那一格不是状态码，别当响应列出来。 */
const METHOD_KEYS = new Set([
  "get", "post", "put", "patch", "delete", "head", "options", "trace",
]);

export function opDetail(
  pathLevel: RawParam[] | undefined,
  op: RawOp,
): OpDetail {
  const seen = new Set<string>();
  const params: OpDetail["params"] = [];
  for (const p of [...(pathLevel ?? []), ...(op.parameters ?? [])]) {
    if (!p?.name) continue;
    const key = `${p.in ?? ""}:${p.name}`;
    if (seen.has(key)) continue;
    seen.add(key);
    params.push({
      name: p.name,
      loc: p.in ?? "query",
      required: p.required === true,
      type: p.schema?.type ?? "",
    });
  }
  const codes = Object.keys(op.responses ?? {}).filter(
    (c) => !METHOD_KEYS.has(c.toLowerCase()),
  );
  return { params, codes: codes.sort() };
}

/** 「展开」第一行：`name（in, required, type）`，空集回界面给的占位句。
 *  拆成函数是为了把这段三元嵌套从 JSX 里请出去（页面有 80 列门）。 */
export function paramsLine(
  params: OpDetail["params"],
  emptyText: string,
): string {
  if (params.length === 0) return emptyText;
  return params
    .map(
      (p) =>
        `${p.name}（${p.loc}${p.required ? ", required" : ""}` +
        `${p.type ? `, ${p.type}` : ""}）`,
    )
    .join("、");
}

/** 「展开」第二行：已声明的状态码；一个没写就回占位句，不假装有 200。 */
export function codesLine(
  codes: string[],
  emptyText: string,
  label: string,
): string {
  if (codes.length === 0) return emptyText;
  return `${label}: ${codes.join(", ")}`;
}

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
    // path item 上的 parameters 对本 path 的每个方法都生效，得并进详情
    const pathLevel = (ops as { parameters?: RawParam[] }).parameters;
    for (const [verb, op] of Object.entries(ops ?? {})) {
      if (!VERBS.includes(verb.toLowerCase()) || typeof op !== "object") {
        continue;
      }
      const tag = op.tags?.[0] || ungrouped;
      const arr = buckets.get(tag) ?? [];
      arr.push({
        path,
        method: verb.toUpperCase(),
        summary: op.summary ?? "",
        detail: opDetail(pathLevel, op),
      });
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
