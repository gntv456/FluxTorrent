import { getDict } from "@/i18n/server";
import { rawFetchHelpers } from "@/lib/api-client";
import { fmt } from "@/i18n/config";
import { PageHeader } from "@/components/ui/page-header";
import { ReadState } from "@/components/ui/read-state";
import {
  groupEndpoints,
  unwrapSpec,
  type Endpoint,
} from "@/lib/openapi-groups";

export const dynamic = "force-dynamic";

/**
 * 开放 API 文档（0283 P2）：直挂 openapi.json 的只读浏览页。
 * 不引外域 CDN（CSP script-src 'self' 且离线部署友好）——按 tag 分组成
 * 端点表，右上角链接直指 JSON 源文件。
 */

function MethodBadge({ method }: { method: string }) {
  const cls = `apidocs-badge apidocs-badge--${method.toLowerCase()}`;
  return <span className={cls}>{method}</span>;
}

interface Spec {
  title: string;
  version: string;
  groups: { tag: string; items: Endpoint[] }[];
  total: number;
  failed: boolean;
}

/** openapi.json 也走全站统一的 ok() 信封 ⇒ 规范本体在 `data` 里。
 *
 *  旧写法读的是 `json.paths` / `json.info`：信封下这两个键恒为 undefined，
 *  于是现网 163 个 operation 被算成 0 个，整页只剩一句「API 暂不可达」。
 *  拆出去的 unwrapSpec + groupEndpoints（lib/openapi-groups.ts）由
 *  tests/openapi-envelope.test.ts 钉住「带信封 / 不带信封 / 非动词键」
 *  三种输入，防止这类漏剥再被静默吞掉。 */
async function loadSpec(ungrouped: string): Promise<Spec> {
  const empty: Spec = {
    title: "",
    version: "",
    groups: [],
    total: 0,
    failed: true,
  };
  try {
    const base = rawFetchHelpers.base();
    const res = await fetch(`${base}/api/v1/openapi.json`, {
      cache: "no-store",
    });
    if (!res.ok) return empty;
    const spec = unwrapSpec(await res.json());
    const { groups, total } = groupEndpoints(spec, ungrouped);
    return {
      title: spec.info?.title ?? "",
      version: spec.info?.version ?? "",
      groups,
      total,
      failed: false,
    };
  } catch {
    return empty;
  }
}

export default async function ApiDocsPage() {
  const { dict } = await getDict();
  const t = dict.apidocs;
  const spec = await loadSpec(t.ungrouped);

  return (
    <div className="flex flex-col gap-4">
      <PageHeader
        eyebrow={t.eyebrow}
        title={t.title}
        sub={t.sub}
        aside={
          <span className="sticker">
            {fmt(t.badgeCount, { n: spec.total, g: spec.groups.length })}
          </span>
        }
      />

      <div className="flex flex-wrap items-center gap-3 text-xs text-sub">
        <span>{t.intro}</span>
        {spec.version !== "" && (
          <span className="num">OpenAPI {spec.version}</span>
        )}
        <a href="/api/v1/openapi.json" className={LINK_SRC}>
          {t.viewSource}
        </a>
      </div>

      <ReadState
        failed={spec.failed}
        empty={spec.total === 0}
        dict={dict}
        emptyTitle={t.emptyTitle}
        emptyDesc={t.emptyDesc}
        emptyIcon="doc"
      >
        <div className="flex flex-col gap-3">
          {spec.groups.map((g) => (
            <section key={g.tag} className="brand-panel overflow-hidden">
              <div className="brand-panel__head">
                <h2>{g.tag}</h2>
                <span className="text-xs text-sub">
                  {fmt(t.groupCount, { n: g.items.length })}
                </span>
              </div>
              <div className="overflow-x-auto">
                <table className="apidocs-table">
                  <thead>
                    <tr>
                      <th>{t.colMethod}</th>
                      <th>{t.colPath}</th>
                      <th>{t.colSummary}</th>
                    </tr>
                  </thead>
                  <tbody>
                    {g.items.map((e) => (
                      <tr key={`${e.method}-${e.path}`}>
                        <td>
                          <MethodBadge method={e.method} />
                        </td>
                        <td className="apidocs-path">{e.path}</td>
                        <td>{e.summary || "—"}</td>
                      </tr>
                    ))}
                  </tbody>
                </table>
              </div>
            </section>
          ))}
        </div>
      </ReadState>
    </div>
  );
}

const LINK_SRC =
  "rounded-full border border-line px-3 py-1 font-bold " +
  "hover:border-[var(--sky)] hover:text-[var(--sky)]";
