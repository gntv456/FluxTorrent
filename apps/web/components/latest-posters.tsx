"use client";

import { useEffect, useRef, useState } from "react";
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
  <text x='150' y='228' font-size='64'
    text-anchor='middle' opacity='0.92'>🎬</text>
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
      className={
        "poster-card group relative block w-full shrink-0 overflow-hidden " +
        "rounded-[var(--r-md)] border border-line bg-[var(--surface-card)] " +
        "shadow-[var(--shadow-card)]"
      }
    >
      {/* 海报图（无图回退生成式海报；挂图由 PosterImage onError 兜底） */}
      <PosterImage
        src={item.poster || posterArt(item.id)}
        alt={item.name}
        size="lg"
      />
      {/* 悬停信息浮层：影视名称 + 豆瓣评分 */}
      <div
        className={
          "pointer-events-none absolute inset-0 flex flex-col justify-end " +
          "bg-gradient-to-t from-black/85 via-black/30 to-transparent p-2.5 " +
          "opacity-0 transition-opacity duration-200 group-hover:opacity-100"
        }
      >
        <p className="line-clamp-2 text-[12px] font-bold leading-snug">
          <span className="text-white">{item.name}</span>
        </p>
        {/* 豆瓣评级星标：内容品牌色（豆瓣绿字黄星口径），W-36 内容令牌 */}
        <p
          className={
            "mt-1 text-[12px] font-bold " +
            "text-[var(--content-rating-star)]"
          }
        >
          {hasRating
            ? `★ ${doubanLabel} ${ratingNum.toFixed(1)}`
            : noRatingLabel}
        </p>
      </div>
    </Link>
  );
}

/** 无缝循环铺轨：把内容重复 N 份直到轨道宽 ≥ 2×容器宽。
 *
 *  无缝的两个必要条件（2026-11-15 修复）：
 *  1. translateX(-50%) 对齐——轨道=整份数组重复整数遍，-50% 时恰好落在
 *     某一遍的开头，循环重置无跳变。旧实现只复制一份，条数少时轨道比
 *     屏还窄，滑到一半右边全空再瞬间跳回＝肉眼断裂。
 *  2. gap 不进 50%——flex gap 会让接缝处少一个 gap（每圈横跳 12px）。
 *     改用组内 margin：每张卡右缘留 gap（末张也是），一半宽度严格=
 *     「一遍的宽度」，接缝与组内间距完全一致。 */
function useLoopCopy(items: PosterItem[], enabled: boolean) {
  const [copies, setCopies] = useState(2);
  const wrapRef = useRef<HTMLDivElement>(null);
  useEffect(() => {
    if (!enabled || items.length === 0) return;
    const CARD_W = 168 + 12; // sm 卡宽 + 组内间距（跟 render 侧常量一致）
    const calc = () => {
      const need = wrapRef.current?.clientWidth ?? 0;
      if (need <= 0) return;
      const one = items.length * CARD_W;
      // 轨道 ≥ 2×容器 ⇒ -50% 之后仍然铺满右侧，无空白无跳变
      setCopies(Math.max(2, Math.ceil((need * 2) / one)));
    };
    calc();
    const ro = new ResizeObserver(calc);
    if (wrapRef.current) ro.observe(wrapRef.current);
    return () => ro.disconnect();
  }, [enabled, items.length]);
  return { copies, wrapRef };
}

/** 轨道渲染：copies 遍 × 每卡右缘留 gap（末张也有 ⇒ -50% 严格对齐） */
function Track({
  items,
  copies,
  cardClass,
  cardW,
  duration,
  renderItem,
}: {
  items: PosterItem[];
  copies: number;
  cardClass: string;
  cardW: string;
  duration: string;
  renderItem: (t: PosterItem, i: number) => React.ReactNode;
}) {
  return (
    <div
      className="poster-marquee flex w-max items-stretch px-1 py-1"
      style={
        {
          "--marquee-duration": duration,
        } as React.CSSProperties
      }
    >
      {Array.from({ length: copies }, (_, c) =>
        items.map((t, i) => (
          <div
            key={`${c}-${t.id}-${i}`}
            className={cardClass}
            style={{ width: cardW, marginRight: 12 }}
          >
            {renderItem(t, i)}
          </div>
        )),
      )}
    </div>
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
  const { copies, wrapRef } = useLoopCopy(base, !compact);
  return (
    <div
      ref={wrapRef}
      className="poster-marquee-wrap overflow-x-hidden"
      aria-hidden
    >
      {compact ? (
        <Track
          items={base}
          copies={1}
          cardClass={
            "poster-card relative block shrink-0 overflow-hidden " +
            "rounded-[var(--r-sm)] border border-line " +
            "bg-[var(--surface-card)] shadow-[var(--shadow-card)]"
          }
          cardW="88px"
          duration={`${Math.max(base.length * 4, 24)}s`}
          renderItem={(t) => (
            <PosterImage src={t.poster || posterArt(t.id)} alt="" size="lg" />
          )}
        />
      ) : (
        <Track
          items={base}
          copies={copies}
          cardClass={
            "poster-card relative block shrink-0 overflow-hidden " +
            "rounded-[var(--r-sm)] border border-line " +
            "bg-[var(--surface-card)] shadow-[var(--shadow-card)]"
          }
          cardW="104px"
          duration={`${Math.max(base.length * 4, 24)}s`}
          renderItem={(t) => (
            <PosterImage src={t.poster || posterArt(t.id)} alt="" size="lg" />
          )}
        />
      )}
    </div>
  );
}

/** 首页「最新资源」海报墙：横向滚动。
 *
 *  桌面是匀速无限跑马灯（悬停暂停 + 海报放大显示名称与评分）；手机端改成
 *  原生横滑（CSS 关掉动画）——自动跑的动画在手机上既占位又抢注意力，
 *  而且用户根本停不下来。窄屏只渲染一份（复制是跑马灯无缝的实现手段，
 *  手动滑不需要）。 */
export function LatestPosters({ items }: { items: PosterItem[] }) {
  const { dict } = useI18n();
  const compact = useIsCompact();
  if (items.length === 0) return null;
  const { copies, wrapRef } = useLoopCopy(items, !compact);
  return (
    <div ref={wrapRef} className="poster-marquee-wrap overflow-x-hidden">
      <div className="px-1 py-2">
        {compact ? (
          <div className="flex w-max items-stretch">
            {items.map((t) => (
              <div key={t.id} style={{ marginRight: 12, width: 150 }}>
                <PosterCard
                  item={t}
                  doubanLabel={dict.home.douban}
                  noRatingLabel={dict.home.noRating}
                />
              </div>
            ))}
          </div>
        ) : (
          <Track
            items={items}
            copies={copies}
            cardClass="shrink-0"
            cardW="168px"
            duration={`${Math.max(items.length * 5, 20)}s`}
            renderItem={(t) => (
              <PosterCard
                item={t}
                doubanLabel={dict.home.douban}
                noRatingLabel={dict.home.noRating}
              />
            )}
          />
        )}
      </div>
    </div>
  );
}
