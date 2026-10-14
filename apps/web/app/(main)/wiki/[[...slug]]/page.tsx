import type { Metadata } from "next";
import Link from "next/link";
import { notFound } from "next/navigation";
import { cache } from "react";
import { api, ApiError } from "@/lib/api-client";
import { ErrorPanel } from "@/components/ui/error-panel";
import { requireModule } from "@/components/module-gate";
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

/** W-27 四态口径（同 /top）：取数失败要能诊断，不许演成「页面不存在」。
 *  旧写法两个 loader 都是裸 `catch { return null }` ⇒ 500/网络失败/4101 与
 *  「slug 真不存在」全被 `notFound()` 合成同一个 404。 */
type Load<T> = { data: T; err: null } | { data: null; err: unknown };

/** 只有「确实不存在」才走 404；其余失败交给 ErrorPanel（P-35b：业务码分不出
 *  不存在与暂时读不到，故一并看 HTTP status） */
function isMissing(e: unknown): boolean {
  return e instanceof ApiError && (e.code === 1004 || e.status === 404);
}

const loadIndex = cache(async (): Promise<Load<WikiIndexItem[]>> => {
  try {
    return { data: await api.get<WikiIndexItem[]>("/api/v1/wiki"), err: null };
  } catch (e) {
    return { data: null, err: e };
  }
});

const loadPage = cache(
  async (slug: string): Promise<Load<WikiPage>> => {
    try {
      const url = `/api/v1/wiki/${slug}`;
      return { data: await api.get<WikiPage>(url), err: null };
    } catch (e) {
      return { data: null, err: e };
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
  const { data } = await loadPage(slug[0]);
  return data ? { title: data.title } : {};
}

export default async function WikiView({
  params,
}: {
  params: Promise<{ slug?: string[] }>;
}) {
  const { slug: seg } = await params;
  const { dict } = await getDict();
  const slug = seg?.[0];
  // U1 模块页守卫（与其余 12+ 条路由同口径）：module_wiki=no 时给统一的
  // 「本站未开放」空态。此前本路由靠 loadIndex 吞掉 4101 再 notFound()
  // ⇒ 成员看到的是「页面不存在」，站长关掉 wiki 却没人知道是关掉了。
  const gate = await requireModule("wiki");
  if (gate) return gate;

  // 目录页：visible 页清单（网关在 module_wiki=no 时 4101 → notFound）
  if (!slug) {
    const { data: items, err } = await loadIndex();
    if (err !== null && isMissing(err)) notFound();
    if (err !== null) return <ErrorPanel error={err} retryHref="/wiki" />;
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

  const { data: page, err: pErr } = await loadPage(slug);
  if (pErr !== null && isMissing(pErr)) notFound();
  if (pErr !== null) return <ErrorPanel error={pErr} retryHref="/wiki" />;
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
