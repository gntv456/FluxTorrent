import type { MetadataRoute } from "next";
import { getSiteProfile } from "@/lib/site-profile";

export const dynamic = "force-dynamic";

/** robots.txt（0201）：跟随站点「允许收录」开关。
 *  默认不可收录——私有站被搜索引擎抓走是事故；放开要站长在设置页显式开。 */
export default async function robots(): Promise<MetadataRoute.Robots> {
  const { seo } = await getSiteProfile();
  if (!seo?.indexable) {
    return { rules: [{ userAgent: "*", disallow: "/" }] };
  }
  return {
    rules: [
      {
        userAgent: "*",
        allow: "/",
        // 即便放开收录，也不让爬虫碰到需要登录态与会动账的路径
        disallow: ["/admin/", "/me/", "/api/", "/checkout/", "/messages/"],
      },
    ],
  };
}
