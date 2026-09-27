import type { Metadata } from "next";
import { notFound } from "next/navigation";
import { cache } from "react";
import { api } from "@/lib/api-client";
import { getDict } from "@/i18n/server";
import { PANEL_BAOZI } from "@/lib/ui-classes";

/**
 * 自定义页面（0187 一审 R4.4）：站长在后台造的任意内容页。
 * body 为后端 ammonia 消毒后的 HTML（与公告同防线），此处直接注水渲染。
 * 导航挂接：后台 menu_items 把自定义菜单 url 填 /p/{slug}。
 */

interface CustomPage {
  slug: string;
  title: string;
  body: string;
  updated_at: string;
}

/** 一次请求内 metadata 与正文共用同一次取数（React cache 去重） */
const loadPage = cache(async (slug: string): Promise<CustomPage | null> => {
  try {
    return await api.get<CustomPage>(`/api/v1/pages/${slug}`);
  } catch {
    return null;
  }
});

/** 页面标题用这条页面自己的 title：0201 把 /p/{slug} 写进 sitemap 后复验发现，
 *  落到页面上 `<title>` 仍是全站同一个「站名 · 后缀」——爬虫拿到一串同名页。 */
export async function generateMetadata({
  params,
}: {
  params: Promise<{ slug: string }>;
}): Promise<Metadata> {
  const { slug } = await params;
  const page = await loadPage(slug);
  return page ? { title: page.title } : {};
}

export default async function CustomPageView({
  params,
}: {
  params: Promise<{ slug: string }>;
}) {
  const { slug } = await params;
  const { dict } = await getDict();
  const page = await loadPage(slug);
  if (!page) notFound();
  return (
    <div className="flex flex-col gap-4">
      <div className={PANEL_BAOZI}>
        <h1 className="font-display text-2xl">{page.title}</h1>
        <div className="mt-1 text-xs text-sub">
          {dict.page.updatedAt}: {page.updated_at.slice(0, 10)}
        </div>
        {/* 后端已 ammonia 消毒（剥 script/事件属性/js 协议），管理富文本注水；
         *  M6.3：长文排版——38em 行宽 + table/pre/img 移动兜底（.longform） */}
        <div
          className="home-news-modal__body longform mt-4"
          dangerouslySetInnerHTML={{ __html: page.body }}
        />
      </div>
    </div>
  );
}
