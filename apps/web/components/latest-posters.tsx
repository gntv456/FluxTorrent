"use client";

import Link from "next/link";
import { useI18n } from "@/i18n/client";
import { useIsCompact } from "@/lib/hooks/use-media";
import { PosterImage } from "@/components/poster-image";

/** 海报卡片数据（首页最新种子投影） */
export interface PosterItem {
  id: number;
  name: string;
  rating: string | null;
  poster: string | null;
}

/** 无海报图时的生成式海报：按 id 确定性渐变 + 影片符号（无外部依赖） */
function posterArt(seed: number): string {
  const h1 = (seed * 47) % 360;
  const h2 = (h1 + 80) % 360;
  const svg = `<svg xmlns='http://www.w3.org/2000/svg' width='300' height='420'>
  <defs><linearGradient id='g' x1='0' y1='0' x2='1' y2='1'>
    <stop offset='0' stop-color='hsl(${h1},58%,42%)'/>
    <stop offset='1' stop-color='hsl(${h2},52%,26%)'/>
  </linearGradient></defs>
  <rect width='300' height='420' fill='url(#g)'/>
  <circle cx='242' cy='72' r='46' fill='rgba(255,255,255,0.13)'/>
  <circle cx='52' cy='342' r='78' fill='rgba(0,0,0,0.12)'/>
  <rect x='28' y='52' width='8' height='316' fill='rgba(255,255,255,0.18)'/>
  <rect x='264' y='52' width='8' height='316' fill='rgba(255,255,255,0.18)'/>
  <text x='150' y='228' font-size='64' text-anchor='middle' opacity='0.92'>🎬</text>
</svg>`;
  return `data:image/svg+xml,${encodeURIComponent(svg)}`;
}

function PosterCard({
  item,
  doubanLabel,
  noRatingLabel,
}: {
  item: PosterItem;
  doubanLabel: string;
  noRatingLabel: string;
}) {
  const ratingNum = item.rating ? Number(item.rating) : NaN;
  const hasRating = Number.isFinite(ratingNum) && ratingNum > 0;
  return (
    <Link
      href={`/torrent/${item.id}`}
      title={item.name}
      className="poster-card group relative block w-[150px] shrink-0 overflow-hidden rounded-[var(--r-md)] border border-line bg-[var(--surface-card)] shadow-[var(--shadow-card)] sm:w-[168px]"
    >
      {/* 海报图（无图回退生成式海报；挂图由 PosterImage onError 兜底） */}
      <PosterImage
        src={item.poster || posterArt(item.id)}
        alt={item.name}
        size="lg"
      />
      {/* 悬停信息浮层：影视名称 + 豆瓣评分 */}
      <div className="pointer-events-none absolute inset-0 flex flex-col justify-end bg-gradient-to-t from-black/85 via-black/30 to-transparent p-2.5 opacity-0 transition-opacity duration-200 group-hover:opacity-100">
        <p className="line-clamp-2 text-[12px] font-bold leading-snug text-white">
          {item.name}
        </p>
        {/* 豆瓣评级星标：内容品牌色（豆瓣绿字黄星口径），W-36 内容令牌 */}
        <p
          className="mt-1 text-[12px] font-bold
            text-[var(--content-rating-star)]"
        >
          {hasRating
            ? `★ ${doubanLabel} ${ratingNum.toFixed(1)}`
            : noRatingLabel}
        </p>
      </div>
    </Link>
  );
}

/** 首屏视觉带（决定三，2026-11-03）：页面顶部一条装饰性海报带。
 *  有真实海报时铺真实封面；没有任何可展示资源时铺生成式主题海报
 *  （posterArt + PosterImage，站内既有口径），首屏不退化成纯文字。
 *  纯装饰：不可点（无对应种子），读屏器跳过；复用同一份 latest 数据源，
 *  不新增请求维度。 */
export function PosterRibbon({ items }: { items: PosterItem[] }) {
  const compact = useIsCompact();
  const seeds = Array.from({ length: 10 }, (_, i) => i + 1);
  const base: PosterItem[] =
    items.length > 0
      ? items
      : seeds.map((s) => ({ id: s, name: "", rating: null, poster: null }));
  const loop = compact ? base : [...base, ...base];
  return (
    <div className="poster-marquee-wrap overflow-x-hidden" aria-hidden>
      <div
        className="poster-marquee flex w-max items-stretch gap-2 px-1 py-1"
        style={
          {
            "--marquee-duration": `${Math.max(base.length * 4, 24)}s`,
          } as React.CSSProperties
        }
      >
        {loop.map((t, i) => (
          <div
            key={`${t.id}-${i}`}
            className="poster-card relative block w-[88px] shrink-0
              overflow-hidden rounded-[var(--r-sm)] border border-line
              bg-[var(--surface-card)] shadow-[var(--shadow-card)] sm:w-[104px]"
          >
            <PosterImage src={t.poster || posterArt(t.id)} alt="" size="lg" />
          </div>
        ))}
      </div>
    </div>
  );
}

/** 首页「最新资源」海报墙：横向滚动。
 *
 *  桌面是匀速无限跑马灯（悬停暂停 + 海报放大显示名称与评分）；手机端改成
 *  原生横滑（CSS 关掉动画）——自动跑的动画在手机上既占位又抢注意力，
 *  而且用户根本停不下来。窄屏不复制第二份内容（复制是跑马灯无缝的实现手段，
 *  手动滑不需要），所以按 compact 只渲染一份。 */
export function LatestPosters({ items }: { items: PosterItem[] }) {
  const { dict } = useI18n();
  const compact = useIsCompact();
  if (items.length === 0) return null;
  const loop = compact ? items : [...items, ...items];
  return (
    <div className="poster-marquee-wrap overflow-x-hidden">
      <div
        className="poster-marquee flex w-max items-stretch gap-3 px-1 py-2"
        style={
          {
            "--marquee-duration": `${Math.max(items.length * 5, 20)}s`,
          } as React.CSSProperties
        }
      >
        {loop.map((t, i) => (
          <PosterCard
            key={`${t.id}-${i}`}
            item={t}
            doubanLabel={dict.home.douban}
            noRatingLabel={dict.home.noRating}
          />
        ))}
      </div>
    </div>
  );
}
