import type { Metadata } from "next";
import { notFound } from "next/navigation";
import { cache } from "react";
import { api, ApiError } from "@/lib/api-client";
import { getDict } from "@/i18n/server";
import { PANEL_BRAND } from "@/lib/ui-classes";
import { PageHeader } from "@/components/ui/page-header";

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

/** 一次请求内 metadata 与正文共用同一次取数（React cache 去重）。
 *  P-313：旧写法裸 `catch { return null }` 把 500/网络失败与「slug 真不存在」
 *  合成同一个 404——而 /p/{slug} 在 sitemap 里，接口抖一下就把真页面从爬虫
 *  索引里摘掉。只有确实 404/1004 才返回 null（→ notFound），其余失败照抛，
 *  交给路由错误边界，不再伪装成「页面不存在」。 */
const loadPage = cache(async (slug: string): Promise<CustomPage | null> => {
  const url = `/api/v1/pages/${slug}`;
  try {
    return await api.get<CustomPage>(url);
  } catch (e) {
    if (e instanceof ApiError && (e.code === 1004 || e.status === 404)) {
      return null;
    }
    throw e;
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
      {/* 页头走 PageHeader：样图这页是「自定义页面 / <标题>」+ 右侧
          最后更新徽章。正文面板里不再重复标题（同一层只留一个 h1，
          评审 §「区块标题层级唯一」）。 */}
      <PageHeader
        eyebrow={dict.page.eyebrow}
        title={page.title}
        sub={dict.page.origin}
        aside={
          <span className="sticker">
            {dict.page.updatedAt} {page.updated_at.slice(0, 10)}
          </span>
        }
      />
      <div className={PANEL_BRAND}>
        {/* 后端已 ammonia 消毒（剥 script/事件属性/js 协议），管理富文本注水；
         *  M6.3：长文排版——38em 行宽 + table/pre/img 移动兜底（.longform） */}
        <div
          className="home-news-modal__body longform"
          dangerouslySetInnerHTML={{ __html: page.body }}
        />
      </div>
    </div>
  );
}
