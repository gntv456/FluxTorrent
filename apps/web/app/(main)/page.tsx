import Link from "next/link";
import { api, paged } from "@/lib/api-client";
import { getDict } from "@/i18n/server";
import { getSiteProfile } from "@/lib/site-profile";
import type { Page, TorrentListItem } from "@fluxtorrent/domain-types";
import { HomeSections } from "@/components/home-sections";
import { LatestPosters } from "@/components/latest-posters";
import {
  parseHomeLayout,
  type HomeSectionMeta,
} from "@/components/home-layout";

export const dynamic = "force-dynamic";

/** 首页（参考站 index.php 像素级复刻）：
 *  社区新鲜事 + 签到日历 → 新增资源统计图表 → 站点数据三列 + 幸运大转盘 → 免责/友链 → 最新种子海报墙 */
export default async function HomePage() {
  const { dict } = await getDict();
  // showcase 模块键（0209 P2-18 接真）：海报墙此前只受排版控制，模块键空转——
  // 现在关 showcase = 不拉不渲染（与其它模块同口径）
  const profile = await getSiteProfile().catch(() => null);
  const showcaseOn = profile?.modules?.showcase !== false;
  let latest: Page<TorrentListItem> | null = null;
  // 置顶促销公告条（0041 sticky_promotions 的前台消费端；空/失败静默隐藏）
  let promos: {
    id: number;
    title: string;
    url: string | null;
    badge: string | null;
  }[] = [];
  try {
    promos = await api.get<
      { id: number; title: string; url: string | null; badge: string | null }[]
    >("/api/v1/sticky-promos");
  } catch {
    promos = [];
  }
  // 首页排版（0089 + 四审 L6 单源化）：是否显示海报墙、以及它排在第几格、占多宽，
  // 全部由同一份排版解析结果决定（清单来自 /home.home_sections，解析函数与
  // HomeSections / 后台编辑器共用），页面不再自己数 key、也不再硬拼末尾位置。
  let showLatest = true;
  try {
    const home = await api.get<{
      home_layout?: string;
      home_sections?: HomeSectionMeta[];
    }>("/api/v1/home");
    const layout = parseHomeLayout(
      home.home_layout,
      home.home_sections ?? [],
    );
    if ((home.home_sections?.length ?? 0) > 0 || home.home_layout?.trim())
      showLatest = layout.some((x) => x.key === "latest");
  } catch {
    // home 接口失败（未登录之外的异常）按默认渲染
  }
  if (showLatest && showcaseOn) {
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
      {/* 板块顺序/占宽由 HomeSections 按排版清单统一决定；海报墙内容在这里
          服务端取好（首屏不退化成客户端取数），只把节点交出去占位。 */}
      <HomeSections
        latest={
          latest === null
            ? null
            : latest.items.length > 0
              ? (
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
              )
              : (
                <table className="nexus-table">
                  <thead>
                    <tr>
                      <td className="colhead">{dict.home.latest}</td>
                    </tr>
                  </thead>
                  <tbody>
                    <tr>
                      <td className="p-6 text-center text-sub">
                        {dict.home.empty}
                      </td>
                    </tr>
                  </tbody>
                </table>
              )
        }
      />
    </div>
  );
}
