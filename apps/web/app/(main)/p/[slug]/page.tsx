import { notFound } from "next/navigation";
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

export default async function CustomPageView({
  params,
}: {
  params: Promise<{ slug: string }>;
}) {
  const { slug } = await params;
  const { dict } = await getDict();
  let page: CustomPage | null = null;
  try {
    page = await api.get<CustomPage>(`/api/v1/pages/${slug}`);
  } catch {
    page = null;
  }
  if (!page) notFound();
  return (
    <div className="flex flex-col gap-4">
      <div className={PANEL_BAOZI}>
        <h1 className="font-display text-2xl">{page.title}</h1>
        <div className="mt-1 text-xs text-sub">
          {dict.page.updatedAt}: {page.updated_at.slice(0, 10)}
        </div>
        {/* 后端已 ammonia 消毒（剥 script/事件属性/js 协议），管理富文本注水 */}
        <div
          className="home-news-modal__body mt-4"
          dangerouslySetInnerHTML={{ __html: page.body }}
        />
      </div>
    </div>
  );
}
