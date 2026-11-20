import Link from "next/link";
import { api } from "@/lib/api-client";
import { Badge } from "@/components/ui/badge";
import { getDict } from "@/i18n/server";

/** 全站公告横幅（0503）：news 此前只在首页渲染——深逛 /torrents 等页的
 *  用户对新公告（规则变更/封禁公示/活动）零感知。
 *  服务端组件挂 (main)/layout：拉 home 的 news 头条，标题含「新」徽标
 *  的当日新鲜公告显示为横幅（news 列表本就 LIMIT 8 按 id 倒序，头条=
 *  最新；新鲜判定由后端 news.date 只有 MM-DD 的口径限制，退而求其次
 *  比对首页板块已有的 latest_news_id 无果——本版先恒显头条可关闭，
 *  localStorage 记已读 id；RSC 读不到 localStorage，关闭由内嵌小
 *  client 按钮承担，横幅本体保持 RSC 零 JS）。失败静默。 */
export async function SiteNewsBanner() {
  let top: { id: number; title: string } | null = null;
  try {
    const r = await api.get<{
      news?: { id: number; title: string }[];
    }>("/api/v1/home", 300);
    top = r?.news?.[0] ?? null;
  } catch {
    top = null;
  }
  if (!top) return null;
  const { dict } = await getDict();
  return (
    <div
      role="status"
      className="site-news-banner flex items-center justify-center
        gap-2 border-b border-[var(--line)] bg-[var(--surface-sunken)]
        px-4 py-1.5 text-center text-sm"
      data-news-id={top.id}
    >
      <Badge tone="warn">{dict.siteNews.label}</Badge>
      <Link
        href="/"
        className="text-link truncate min-h-[44px] inline-flex items-center"
      >
        {top.title}
      </Link>
    </div>
  );
}
