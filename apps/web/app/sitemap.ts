import type { MetadataRoute } from "next";
import { api } from "@/lib/api-client";
import { getSiteProfile } from "@/lib/site-profile";
import { siteBase } from "@/lib/site-url";

export const dynamic = "force-dynamic";

interface PublicPage {
  slug: string;
  updated_at: string;
}

/** sitemap.xml（0201）。两条口径：
 *  1) 只在「允许收录」打开时给出条目——没放开的站提交 sitemap 是自相矛盾；
 *  2) 只列**匿名可访问**的地址：首页 + 公开自定义页。种子详情/列表需要登录态
 *     （匿名打 /torrents 是 401），把它们写进去等于给爬虫一堆死链。
 *  绝对地址依赖 PUBLIC_SITE_URL（与 RSS、支付回调同一口径）；未配置时
 *  Next 会告警并输出相对路径，属部署缺项而非本文件的问题。 */
export default async function sitemap(): Promise<MetadataRoute.Sitemap> {
  const { seo } = await getSiteProfile();
  if (!seo?.indexable) return [];
  const base = siteBase();
  const pages = await api
    .get<PublicPage[]>("/api/v1/public-pages")
    .catch((): PublicPage[] => []);
  const entries: MetadataRoute.Sitemap = [
    { url: base ? new URL("/", base).href : "/", changeFrequency: "daily" },
    ...pages.map((p) => ({
      url: base ? new URL(`/p/${p.slug}`, base).href : `/p/${p.slug}`,
      lastModified: p.updated_at,
      changeFrequency: "weekly" as const,
    })),
  ];
  return entries;
}
