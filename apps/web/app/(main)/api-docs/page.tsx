import { getDict } from "@/i18n/server";
import { rawFetchHelpers } from "@/lib/api-client";

export const dynamic = "force-dynamic";

/**
 * 开放 API 文档（0283 P2）：直挂 openapi.json 的只读浏览页。
 * 不引外域 CDN（CSP script-src 'self' 且离线部署友好）——渲染成
 * 分组端点表 + 展开方法/路径/摘要，链接直指 JSON 源文件。
 */

function MethodBadge({ method }: { method: string }) {
  const cls = `apidocs-badge apidocs-badge--${method.toLowerCase()}`;
  return <span className={cls}>{method}</span>;
}

interface Doc {
  dictTitle: string;
  endpoints: { path: string; method: string; summary: string }[];
}

export default async function ApiDocsPage() {
  const { dict } = await getDict();
  let doc: Doc = { dictTitle: "", endpoints: [] };
  try {
    const base = rawFetchHelpers.base();
    const res = await fetch(`${base}/api/v1/openapi.json`, {
      cache: "no-store",
    });
    const json = res.ok ? await res.json() : null;
    const endpoints: Doc["endpoints"] = [];
    for (const [path, methods] of Object.entries(json.paths ?? {})) {
      for (const [method, op] of Object.entries(methods as object)) {
        if (typeof op === "object" && op) {
          endpoints.push({
            path,
            method: method.toUpperCase(),
            summary: (op as { summary?: string }).summary ?? "",
          });
        }
      }
    }
    const byPath = (a: { path: string; method: string }, b: typeof a) =>
      a.path.localeCompare(b.path) || a.method.localeCompare(b.method);
    endpoints.sort(byPath);
    doc = { dictTitle: json.info?.title ?? "API", endpoints };
  } catch {
    // API 不可达：空表
  }
  return (
    <div className="flex flex-col gap-4">
      <h1 className="font-display text-2xl">{dict.apidocs.title}</h1>
      <p className="text-sm text-muted">
        {dict.apidocs.intro}{" "}
        <a href="/api/v1/openapi.json" className="underline">
          openapi.json
        </a>
      </p>
      <div className="overflow-x-auto">
        <table className="apidocs-table">
          <thead>
            <tr>
              <th>{dict.apidocs.colMethod}</th>
              <th>{dict.apidocs.colPath}</th>
              <th>{dict.apidocs.colSummary}</th>
            </tr>
          </thead>
          <tbody>
            {doc.endpoints.map((e) => (
              <tr key={`${e.method}-${e.path}`}>
                <td>
                  <MethodBadge method={e.method} />
                </td>
                <td className="apidocs-path">{e.path}</td>
                <td>{e.summary}</td>
              </tr>
            ))}
          </tbody>
        </table>
      </div>
      {doc.endpoints.length === 0 && (
        <p className="text-sm text-muted">{dict.apidocs.unavailable}</p>
      )}
    </div>
  );
}
