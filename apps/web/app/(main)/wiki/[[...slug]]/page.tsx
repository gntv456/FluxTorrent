import type { Metadata } from "next";
import Link from "next/link";
import { notFound } from "next/navigation";
import { cache } from "react";
import { api } from "@/lib/api-client";
import { getDict } from "@/i18n/server";
import { PANEL_BAOZI } from "@/lib/ui-classes";
import { hasBBCode, renderBBCode } from "@/lib/bbcode";
import { MarkdownRenderer } from "@/components/forum-markdown";

/**
 * Wiki 公开页（0359）：无 slug = 目录页，有 slug = 正文页。
 *
 * 正文是 BBCode——renderBBCode 渲染成 React 节点（无 innerHTML 注水点，
 * 天然免疫 XSS），这是与 custom_pages（HTML + ammonia 消毒 + 注水）
 * 的关键差异，后端出站不做消毒。
 */

interface WikiIndexItem {
  id: number;
  slug: string;
  title: string;
}

interface WikiPage {
  slug: string;
  title: string;
  body: string;
  updated_at: string;
}

const loadIndex = cache(async (): Promise<WikiIndexItem[] | null> => {
  try {
    return await api.get<WikiIndexItem[]>("/api/v1/wiki");
  } catch {
    return null;
  }
});

const loadPage = cache(
  async (slug: string): Promise<WikiPage | null> => {
    try {
      return await api.get<WikiPage>(`/api/v1/wiki/${slug}`);
    } catch {
      return null;
    }
  },
);

export async function generateMetadata({
  params,
}: {
  params: Promise<{ slug?: string[] }>;
}): Promise<Metadata> {
  const { slug } = await params;
  if (!slug?.length) return {};
  const page = await loadPage(slug[0]);
  return page ? { title: page.title } : {};
}

export default async function WikiView({
  params,
}: {
  params: Promise<{ slug?: string[] }>;
}) {
  const { slug: seg } = await params;
  const { dict } = await getDict();
  const slug = seg?.[0];

  // 目录页：visible 页清单（网关在 module_wiki=no 时 4101 → notFound）
  if (!slug) {
    const items = await loadIndex();
    if (items === null) notFound();
    return (
      <div className="flex flex-col gap-4">
        <div className={PANEL_BAOZI}>
          <h1 className="font-display text-2xl">{dict.wiki.title}</h1>
          <ul className="mt-4 flex flex-col gap-2">
            {items.map((p) => (
              <li key={p.id}>
                <Link
                  href={`/wiki/${p.slug}`}
                  className="underline-offset-2 hover:underline"
                >
                  {p.title}
                </Link>
              </li>
            ))}
            {items.length === 0 && (
              <li className="text-sm text-sub">{dict.wiki.empty}</li>
            )}
          </ul>
        </div>
      </div>
    );
  }

  const page = await loadPage(slug);
  if (!page) notFound();
  return (
    <div className="flex flex-col gap-4">
      <div className={PANEL_BAOZI}>
        <h1 className="font-display text-2xl">{page.title}</h1>
        <div className="mt-1 flex items-center gap-3 text-xs text-sub">
          <Link href="/wiki" className="underline">
            {dict.wiki.backIndex}
          </Link>
          <span>
            {dict.wiki.updatedAt}: {page.updated_at.slice(0, 10)}
          </span>
        </div>
        {/* BBCode → React 节点（XSS 天然免疫）；无 BBCode 标签时回落
         *  forum-markdown 的 markdown-lite（与种子简介同口径） */}
        <div className="longform mt-4 text-sm leading-relaxed">
          {hasBBCode(page.body) ? (
            <div className="whitespace-pre-wrap">
              {renderBBCode(page.body)}
            </div>
          ) : (
            <MarkdownRenderer source={page.body} />
          )}
        </div>
      </div>
    </div>
  );
}
