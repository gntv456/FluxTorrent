"use client";

import Link from "next/link";
import { useI18n } from "@/i18n/client";

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
      {/* 海报图 */}
      {/* eslint-disable-next-line @next/next/no-img-element */}
      <img
        src={item.poster || posterArt(item.id)}
        alt={item.name}
        loading="lazy"
        className="aspect-[5/7] w-full object-cover"
      />
      {/* 悬停信息浮层：影视名称 + 豆瓣评分 */}
      <div className="pointer-events-none absolute inset-0 flex flex-col justify-end bg-gradient-to-t from-black/85 via-black/30 to-transparent p-2.5 opacity-0 transition-opacity duration-200 group-hover:opacity-100">
        <p className="line-clamp-2 text-[12px] font-bold leading-snug text-white">
          {item.name}
        </p>
        <p className="mt-1 text-[12px] font-bold text-[#f5c518]">
          {hasRating
            ? `★ ${doubanLabel} ${ratingNum.toFixed(1)}`
            : noRatingLabel}
        </p>
      </div>
    </Link>
  );
}

/** 首页「最新资源」海报墙：匀速横向滚动，悬停暂停 + 海报放大显示名称与评分 */
export function LatestPosters({ items }: { items: PosterItem[] }) {
  const { dict } = useI18n();
  if (items.length === 0) return null;
  const loop = [...items, ...items]; // 双份内容实现无缝循环
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
