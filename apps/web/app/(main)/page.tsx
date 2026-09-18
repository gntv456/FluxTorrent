import Link from "next/link";
import { api, paged } from "@/lib/api-client";
import { getDict } from "@/i18n/server";
import type { Page, TorrentListItem } from "@fluxtorrent/domain-types";
import { HomeSections } from "@/components/home-sections";
import { LatestPosters } from "@/components/latest-posters";

export const dynamic = "force-dynamic";

/** 首页（参考站 index.php 像素级复刻）：
 *  社区新鲜事 + 签到日历 → 新增资源统计图表 → 站点数据三列 + 幸运大转盘 → 免责/友链 → 最新种子海报墙 */
export default async function HomePage() {
  const { dict } = await getDict();
  let latest: Page<TorrentListItem> | null = null;
  // 置顶促销公告条（0041 sticky_promotions 的前台消费端；空/失败静默隐藏）
  let promos: { id: number; title: string; url: string | null; badge: string | null }[] = [];
  try {
    promos = await api.get<
      { id: number; title: string; url: string | null; badge: string | null }[]
    >("/api/v1/sticky-promos");
  } catch {
    promos = [];
  }
  // 首页排版（0089）：自定义布局显式包含 latest 才渲染海报墙（默认布局恒显示）。
  // latest 由本 RSC 渲染（需服务端取数），其余板块在 HomeSections 内按配置排布。
  let layoutRaw: string | undefined;
  let showLatest = true;
  try {
    const home = await api.get<{ home_layout?: string }>("/api/v1/home");
    layoutRaw = home.home_layout;
    if (layoutRaw && layoutRaw.trim()) {
      try {
        const arr = JSON.parse(layoutRaw) as { key?: string }[];
        showLatest = Array.isArray(arr) && arr.some((x) => x?.key === "latest");
      } catch {
        showLatest = true; // 非法配置回默认：显示
      }
    }
  } catch {
    // home 接口失败（未登录之外的异常）按默认渲染
  }
  if (showLatest) {
    try {
      latest = await paged<TorrentListItem>("/api/v1/torrents", { limit: 12 });
    } catch {
      // 后端未启动时首页降级为空态（本地开发体验）
    }
  }

  return (
    <div className="flex flex-col gap-4">
      {/* 置顶促销公告条：管理后台「置顶促销」配置的生效条目（标题可带链接/徽标） */}
      {promos.length > 0 && (
        <ul className="flex flex-col gap-1 rounded-[var(--r-md)] border border-sun/40 bg-sun/10 px-3 py-2">
          {promos.map((p) => (
            <li key={p.id} className="flex items-center gap-2 text-sm">
              {p.badge && (
                <span className="sticker bg-sun text-ink">{p.badge}</span>
              )}
              {p.url ? (
                <a
                  href={p.url}
                  className="font-bold text-sky hover:underline"
                  target={p.url.startsWith("http") ? "_blank" : undefined}
                  rel="noreferrer"
                >
                  {p.title}
                </a>
              ) : (
                <span className="font-bold">{p.title}</span>
              )}
            </li>
          ))}
        </ul>
      )}
      <HomeSections />

      {/* 最新资源海报墙：匀速滚动，悬停暂停 + 放大显示名称与豆瓣评分 */}
      {latest && latest.items.length > 0 && (
        <section className="flex flex-col gap-1">
          <div className="flex items-baseline justify-between px-1">
            <h2 className="font-display">{dict.home.latest}</h2>
            <Link href="/torrents" className="text-xs">
              {dict.home.viewAll}
            </Link>
          </div>
          <LatestPosters
            items={latest.items.map((t) => ({
              id: t.id,
              name: t.name,
              rating: t.rating ?? null,
              poster: t.poster ?? null,
            }))}
          />
        </section>
      )}
      {latest && latest.items.length === 0 && (
        <table className="nexus-table">
          <thead>
            <tr>
              <td className="colhead">{dict.home.latest}</td>
            </tr>
          </thead>
          <tbody>
            <tr>
              <td className="p-6 text-center text-sub">{dict.home.empty}</td>
            </tr>
          </tbody>
        </table>
      )}
    </div>
  );
}
