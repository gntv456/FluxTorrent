"use client";

import Link from "next/link";
import { useState } from "react";
import { useI18n } from "@/i18n/client";
import { useIsCompact } from "@/lib/hooks/use-media";
import { useSwipe } from "@/lib/hooks/use-swipe";
import { promotionBadge } from "@/lib/format";
import { catColor } from "@/lib/site-profile";
import { formatBytes } from "@/lib/format";
import type { TorrentListItem } from "@fluxtorrent/domain-types";

/** 行卡片最小数据形状（M5.2 下沉共享）：torrents 列表的 TorrentListItem
 *  是超集；my/torrentlist 的 SnatchRow 子集也能喂（poster/promotion 等
 *  可选字段缺省时自动退化为纯色封面 + 无徽标）。 */
export type RowCardItem = {
  id: number;
  name: string;
  size: number;
  seeders: number;
  leechers: number;
  times_completed?: number;
  small_descr?: string | null;
  poster?: string | null;
  promotion?: string | null;
  category_id?: number;
  sec_names?: string[];
};

/** 种子行卡片（M3，移动端方案 §3）：<640 且 URL 未显式指定 view 时，
 *  table 自动落到行卡片（封面 + 两行标题 + 促销徽标 + 单行 mono 数字）。
 *  与桌面表格共用同一份 items（RSC 取数）；本组件只做展示分支，
 *  不动服务端分页、keyset 游标、count 采样与共享缓存键。
 *  促销徽标口径与桌面一致（torrents/promo.rs 唯一源 → promotionBadge）。 */

function SwipeRow({
  t,
  children,
}: {
  t: RowCardItem;
  children: React.ReactNode;
}) {
  const [open, setOpen] = useState(false);
  const [offset, bind] = useSwipe(open, setOpen);
  void t;
  return (
    <div className="trow-swipe">
      <div className="trow-swipe__acts">
        <a href={`/torrent/${t.id}?dl=1`} className="trow-swipe__act">
          ⬇
        </a>
        <button
          type="button"
          className="trow-swipe__act is-fav"
          onClick={() => setOpen(false)}
        >
          ☆
        </button>
      </div>
      <div
        className="trow-swipe__front"
        style={offset ? { transform: `translateX(${offset}px)` } : undefined}
        {...bind}
      >
        {children}
      </div>
    </div>
  );
}

export function TorrentRowCard({
  t,
  colors,
}: {
  t: RowCardItem;
  colors?: Record<number, string>;
}) {
  const { dict, locale } = useI18n();
  const d = dict.torrent;
  const promo = promotionBadge((t.promotion ?? null) as never);
  const cat = catColor(colors ?? {}, t.category_id);
  return (
    <SwipeRow t={t}>
    <Link
      href={`/torrent/${t.id}`}
      className="trow-card"
      style={{ ["--trow-cat" as string]: cat }}
    >
      <span className="trow-card__cover" aria-hidden>
        {t.poster ? (
          // eslint-disable-next-line @next/next/no-img-element
          <img src={t.poster} alt="" loading="lazy" />
        ) : (
          <span className="trow-card__cat">{t.sec_names?.[0] ?? ""}</span>
        )}
        {promo && (
          <span className={`trow-card__promo ${promo.className}`}>
            {dict.promotion[promo.key]}
          </span>
        )}
      </span>
      <span className="trow-card__body">
        <span className="trow-card__title">{t.name}</span>
        <span className="trow-card__meta">
          {t.small_descr || (t.sec_names ?? []).slice(0, 2).join(" · ")}
        </span>
        <span className="trow-card__nums num">
          <span>{formatBytes(t.size)}</span>
          <span className="trow-card__seed">↑ {t.seeders}</span>
          <span className="trow-card__leech">↓ {t.leechers}</span>
          <span className="trow-card__done">✓ {t.times_completed}</span>
        </span>
      </span>
    </Link>
    </SwipeRow>
  );
}

/** 紧凑态行卡片列表：SSR 首帧渲染表格（useIsCompact fallback=桌面），
 *  挂载后 <640 切行卡片——与全站 use-media 纪律一致。 */
export function TorrentRowList({
  items,
  colors,
}: {
  items: RowCardItem[];
  colors?: Record<number, string>;
}) {
  const compact = useIsCompact();
  if (!compact) return null;
  return (
    <div className="trow-list">
      {items.map((t) => (
        <TorrentRowCard key={t.id} t={t} colors={colors} />
      ))}
    </div>
  );
}
