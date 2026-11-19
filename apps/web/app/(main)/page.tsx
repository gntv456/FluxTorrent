import Link from "next/link";
/* 首页移动端样式（2026-11-02 重设计）：路由级加载，只服务首页这一条路由
   （同 arcade.css 的纪律：不挂 globals.css 根链让全站 90+ 页都下载）。 */
import "@/app/styles/home-mobile.css";
import "@/app/styles/ux-product.css";
import "@/app/styles/ux-home.css";
import { api, paged } from "@/lib/api-client";
import { getDict } from "@/i18n/server";
import { dateLocale } from "@/i18n/config";
import { getSiteProfile } from "@/lib/site-profile";
import type { Page, TorrentListItem } from "@fluxtorrent/domain-types";
import { HomeSections } from "@/components/home-sections";
import { ShjHero } from "@/components/shj-hero";
import { homeMeSummary, type HomeMeRaw } from "./_parts/home-me-summary";
import type { HomeData } from "@/components/home-data";
import {
  LatestPosters,
  PosterRibbon,
  type PosterItem,
} from "@/components/latest-posters";
import { HomeCategoryGuide } from "@/components/home-category-guide";
import { RecentViewsStrip } from "@/components/recent-views-strip";
import { Badge } from "@/components/ui/badge";
import {
  parseHomeLayout,
  type HomeSectionMeta,
} from "@/components/home-layout";

export const dynamic = "force-dynamic";

/** 首页（参考站 index.php 像素级复刻）：
 *  社区新鲜事 + 签到日历 → 新增资源统计图表 → 站点数据三列 + 幸运大转盘 → 免责/友链 → 最新种子海报墙 */
export default async function HomePage() {
  const { dict, locale } = await getDict();
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
  // M3 移动问候条数据（/me/overview 摘要；失败回落 null 只渲染问候语）
  const ov = await api.get<HomeMeRaw>("/api/v1/me/overview").catch(() => null);
  const meSummary = homeMeSummary(ov, dateLocale(locale));
  const brandName = profile?.brand || dict.common.brand;
  // 驾驶舱多读 username / seeding：沿用同一份已取到的 overview，不新增请求
  const ovx = ov as (HomeMeRaw & {
    username?: string;
    seeding?: number;
    hr?: number;
    hr_unresolved?: number;
  }) | null;
  const meView =
    meSummary && ovx
      ? {
          ...meSummary,
          username: ovx.username ?? "",
          seeding: ovx.seeding ?? 0,
          hr: ovx.hr_unresolved ?? ovx.hr,
        }
      : meSummary;

  // 首页数据（SSR 直出，P1 修复）：一次取回完整 HomeData 传给 HomeSections，
  // 首屏即渲染真内容，不再退化到"加载中…"客户端壳（伤首屏与 SEO）。
  let homeData: HomeData | null = null;
  try {
    homeData = await api.get<HomeData>("/api/v1/home");
  } catch {
    // 后端未启动/异常时降级：HomeSections 客户端会再拉一次
    homeData = null;
  }
  // 首页排版（0089 + 四审 L6 单源化）：是否显示海报墙、以及它排在第几格、占多宽，
  // 全部由同一份排版解析结果决定（清单来自 /home.home_sections，解析函数与
  // HomeSections / 后台编辑器共用），页面不再自己数 key、也不再硬拼末尾位置。
  let showLatest = true;
  if (homeData) {
    const layout = parseHomeLayout(
      homeData.home_layout,
      homeData.home_sections ?? [],
    );
    if (
      (homeData.home_sections?.length ?? 0) > 0 ||
      homeData.home_layout?.trim()
    )
      showLatest = layout.some((x) => x.key === "latest");
  }
  if (showLatest && showcaseOn) {
    try {
      latest = await paged<TorrentListItem>("/api/v1/torrents", { limit: 12 });
    } catch {
      // 后端未启动时首页降级为空态（本地开发体验）
    }
  }
  // 已通过审核的资源总数（home.site_data 同源计数）：空态据此区分「冷启动」
  // 与「有资源但都不是活种」两种口径
  const hasApproved = (homeData?.site_data?.torrents ?? 0) > 0;
  /** 首屏视觉带与最新资源海报墙共用同一份投影（不新增请求） */
  const posterItems: PosterItem[] = latest
    ? latest.items.map((t) => ({
        id: t.id,
        name: t.name,
        rating: t.rating ?? null,
        poster: t.poster ?? null,
      }))
    : [];
  /** 空态主视觉：站点顶级分类（来自已取的档案，无新增请求） */
  const topCategories = (profile?.categories ?? [])
    .filter((c) => !c.parent_id)
    .slice(0, 8)
    .map((c) => ({ id: c.id, name: c.name }));

  return (
    <div className="home-shell--mobile flex flex-col gap-4">
      {/* 置顶促销公告条：管理后台「置顶促销」配置的生效条目（标题可带链接/徽标） */}
      {promos.length > 0 && (
        <ul
      className="flex flex-col gap-1 rounded-[var(--r-md)] border
                 border-[var(--tone-line)] bg-[var(--surface-sunken)]
                 px-3 py-2"
    >
          {promos.map((p) => (
            <li key={p.id} className="flex items-center gap-2 text-sm">
              {p.badge && <Badge tone="warn">{p.badge}</Badge>}
              {p.url ? (
                <a
                  href={p.url}
                  className="font-bold text-link hover:underline"
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
      {/* S1 语义修复：首页模块化布局无自然主标题——补唯一 sr-only h1
          （各区块头已由 h1 降为 h2），供 SEO 与读屏器锚定页面主题。 */}
      <h1 className="sr-only">{brandName}</h1>
      {/* PM3-1：最近浏览续接条（有记录才渲染；纯本地，零请求） */}
      <RecentViewsStrip />
      {/* 山海经 Hero「山海图卷」：恒渲染、CSS 控显隐（其余三主题 display:none
          零布局影响）。玉牌数据与「站点数据」区块同源（home.site_data） */}
      <ShjHero
        brand={brandName}
        themeName={dict.common.themeShanhaijing}
        kicker={dict.home.shjKicker}
        sub={dict.home.shjSub}
        jades={
          homeData?.site_data
            ? [
                {
                  label: dict.home.shjJadeUsers,
                  value: homeData.site_data.users.toLocaleString(locale),
                },
                {
                  label: dict.home.shjJadeTorrents,
                  value: homeData.site_data.torrents.toLocaleString(locale),
                },
                {
                  label: dict.home.shjJadeSeeders,
                  value: homeData.site_data.seeders.toLocaleString(locale),
                },
                {
                  label: dict.home.shjJadePeers,
                  value: homeData.site_data.peers.toLocaleString(locale),
                },
              ]
            : []
        }
      />
      {/* 首屏视觉带：有真实封面铺封面；空数据改分类引导卡，不再摆生成式假海报 */}
      {posterItems.length > 0 ? (
        <PosterRibbon items={posterItems} />
      ) : (
        <HomeCategoryGuide categories={topCategories} />
      )}
      <HomeSections
        initialData={homeData}
        initialMods={profile?.modules ?? {}}
        me={meView}
        brand={brandName}
        latest={
          latest === null ? null : latest.items.length > 0 ? (
            <section className="flex flex-col gap-1">
              <div className="flex items-baseline justify-between px-1">
                <h2 className="font-display">{dict.home.latest}</h2>
                <Link href="/torrents" className="text-xs">
                  {dict.home.viewAll}
                </Link>
              </div>
              <LatestPosters items={posterItems} />
            </section>
          ) : (
            <section className="ux-latest-empty">
              <h2 className="ux-latest-empty__head">{dict.home.latest}</h2>
              {/* 主视觉=分类引导（不误导搜索用户）；发布/看活种降为次要动作 */}
              <HomeCategoryGuide categories={topCategories} compact />
              <p className="ux-latest-empty__note">
                {hasApproved
                  ? dict.torrent.emptyAliveTitle
                  : dict.home.empty}
              </p>
              <div className="ux-latest-empty__actions">
                {hasApproved ? (
                  <Link
                    href="/torrents?alive=0"
                    className="ux-btn ux-btn--ghost"
                  >
                    {dict.torrent.emptyAliveAction}
                  </Link>
                ) : (
                  <Link href="/upload" className="ux-btn ux-btn--ghost">
                    {dict.home.emptyAction}
                  </Link>
                )}
              </div>
            </section>
          )
        }
      />
    </div>
  );
}
