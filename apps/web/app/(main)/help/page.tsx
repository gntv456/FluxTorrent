import type { Metadata } from "next";
import Link from "next/link";
import { cache } from "react";
import { api } from "@/lib/api-client";
import { getDict } from "@/i18n/server";
import { PANEL_BAOZI } from "@/lib/ui-classes";

/** 帮助中心目录（0274）：读 GET /api/v1/help-docs（doc_group 非空的可见页）。
 *  单篇正文复用 /p/{slug}——正文渲染、消毒、深色适配全走那条既有链路，
 *  本页只负责「列出有哪些文档 + 分组」。数据源是 DB，站长在后台
 *  「自定义页面」面板改 slug 前缀 help-* 的那些条目即可（DB 是权威源，
 *  仓库里的 docs/user/*.md 只是导入源，见 scripts/import_user_docs.py）。 */

export const dynamic = "force-dynamic";

interface HelpDoc {
  slug: string;
  title: string;
  group: string;
  sort: number;
}

const loadDocs = cache(async (): Promise<HelpDoc[]> => {
  try {
    return await api.get<HelpDoc[]>("/api/v1/help-docs");
  } catch {
    return [];
  }
});

export async function generateMetadata(): Promise<Metadata> {
  const { dict } = await getDict();
  return { title: dict.help.title };
}

export default async function HelpPage() {
  const { dict } = await getDict();
  const docs = await loadDocs();

  // 按 group 聚合，组内保持后端给定的 doc_sort 顺序
  const groups = new Map<string, HelpDoc[]>();
  for (const d of docs) {
    const list = groups.get(d.group) ?? [];
    list.push(d);
    groups.set(d.group, list);
  }

  return (
    <div className="flex flex-col gap-4">
      <div className="pghd">
        <div>
          <div className="pg-eyebrow">{dict.help.groupGuide}</div>
          <h1 className="font-display text-2xl">{dict.help.title}</h1>
          <p className="mt-1 text-xs text-sub">{dict.help.subtitle}</p>
        </div>
      </div>

      {docs.length === 0 ? (
        <div className={PANEL_BAOZI}>
          <p className="text-sm text-sub">{dict.help.empty}</p>
        </div>
      ) : (
        [...groups.entries()].map(([group, list]) => (
          <section key={group} className="baozi-panel">
            <div className="baozi-panel__head">
              <h2>{dict.help.groupGuide}</h2>
            </div>
            <ul className="flex flex-col gap-2">
              {list.map((d) => (
                <li key={d.slug}>
                  <Link
                    href={`/p/${d.slug}`}
                    className="flex items-center justify-between gap-3 \
rounded-lg px-3 py-2 hover:bg-soft"
                  >
                    <span className="text-sm">{d.title}</span>
                    <span className="shrink-0 text-xs text-sub">
                      {dict.help.readDoc}
                    </span>
                  </Link>
                </li>
              ))}
            </ul>
          </section>
        ))
      )}
    </div>
  );
}
