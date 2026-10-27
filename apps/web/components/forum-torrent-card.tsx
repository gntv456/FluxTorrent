"use client";

import { useEffect, useState } from "react";
import Link from "next/link";
import { api } from "@/lib/api-client";
import { formatBytes, promotionBadge } from "@/lib/format";
import { useI18n } from "@/i18n/client";
import { PosterImage } from "@/components/poster-image";

/**
 * 论坛种子引用卡（0189 视频内嵌的同族块）：`!torrent(id)` 独立行语法或
 * 整行恰为站内种子链接（/torrent/{id} unfurl）→ 横向摘要卡（海报/名称/
 * 大小/做种数/促销标）。数据取现成 GET /torrents/{id}（登录态主键点查），
 * fail-closed：取数失败（不存在/不可见/网络）降级为「已失效」虚线框。
 */

/** 卡片用种子摘要（TorrentRow 子集；promo/seeders 由详情接口单点下发） */
type CardRow = {
  id: number;
  name: string;
  small_descr?: string | null;
  size: number;
  seeders: number;
  leechers: number;
  promotion?: import("@fluxtorrent/domain-types").PromotionKind | null;
  poster?: string | null;
};

/** 模块级取数缓存：同页多帖引用同一种子只打一次接口；null = 已失效 */
const cardCache = new Map<number, CardRow | null>();

/** 失效/不可见兜底：暗淡虚线框 + 保留链接（fail-closed，不出错态 UI） */
function TorrentCardGone({ id, label }: { id: number; label: string }) {
  return (
    <Link
      href={`/torrent/${id}`}
      className={
        "my-2 flex items-center gap-2 rounded-[var(--r-md)] " +
        "border border-dashed border-line px-3 py-2 text-sm " +
        "text-sub hover:border-sky"
      }
    >
      <span aria-hidden>🧲</span>
      <span>{label}</span>
      <span className="text-xs">#{id}</span>
    </Link>
  );
}

/** 骨架条（取数中） */
function TorrentCardSkeleton() {
  return (
    <div
      className={
        "my-2 flex animate-pulse items-center gap-3 " +
        "rounded-[var(--r-md)] border border-line p-2"
      }
      aria-hidden
    >
      <div
        className={
          "h-[46px] w-[31px] shrink-0 rounded " +
          "bg-[var(--surface-sunken)]"
        }
      />
      <div className="flex w-full flex-col gap-1.5">
        <div className="h-3.5 w-3/5 rounded bg-[var(--surface-sunken)]" />
        <div className="h-3 w-2/5 rounded bg-[var(--surface-sunken)]" />
      </div>
    </div>
  );
}

export function TorrentRefBlock({ id }: { id: number }) {
  const { dict } = useI18n();
  const [row, setRow] = useState<CardRow | null | undefined>(
    cardCache.has(id) ? cardCache.get(id) : undefined,
  );
  useEffect(() => {
    if (cardCache.has(id)) return;
    let alive = true;
    api
      .get<CardRow>(`/api/v1/torrents/${id}`)
      .then((r) => {
        if (!alive) return;
        cardCache.set(id, r);
        setRow(r);
      })
      .catch(() => {
        if (!alive) return;
        cardCache.set(id, null);
        setRow(null);
      });
    return () => {
      alive = false;
    };
  }, [id]);

  if (row === null) {
    return <TorrentCardGone id={id} label={dict.forums.torrentGone} />;
  }
  if (!row) return <TorrentCardSkeleton />;

  const dTorrent = dict.torrent;
  const dPromo = dict.promotion;
  const promo = promotionBadge(row.promotion ?? null);

  return (
    <Link
      href={`/torrent/${row.id}`}
      className={
        "my-2 flex items-center gap-3 rounded-[var(--r-md)] " +
        "border border-line bg-[var(--surface-card)] p-2 " +
        "transition-colors hover:border-sky"
      }
    >
      <PosterImage
        src={row.poster ?? undefined}
        alt={row.name}
        size="sm"
        rounded="md"
        className="shrink-0"
      />
      <div className="flex min-w-0 flex-1 flex-col gap-0.5">
        <span className="line-clamp-2 text-sm font-bold text-ink">
          {row.name}
        </span>
        {row.small_descr && (
          <span className="truncate text-xs text-sub">
            {row.small_descr}
          </span>
        )}
        <span
          className={
            "flex flex-wrap items-center gap-x-2 gap-y-0.5 " +
            "text-xs text-sub"
          }
        >
          <span>{formatBytes(row.size)}</span>
          <span aria-hidden>·</span>
          <span>
            {dTorrent.seeding} {row.seeders}
          </span>
          <span aria-hidden>·</span>
          <span>
            {dTorrent.leeching} {row.leechers}
          </span>
          {promo && (
            <span className="torrents-promo torrents-promo--free">
              {dPromo[promo.key]}
            </span>
          )}
        </span>
      </div>
    </Link>
  );
}
