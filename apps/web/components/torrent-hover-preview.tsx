"use client";

import { useEffect, useRef, useState } from "react";
import { api } from "@/lib/api-client";
import { formatBytes } from "@/lib/format";
import { useI18n } from "@/i18n/client";
import type { TorrentListItem } from "@fluxtorrent/domain-types";

/** 列表 hover 预览卡（六维阶段三「批量与键盘」收尾件）：
 *  悬停标题链接 ≥400ms 弹出小卡（副题/促销/统计），移开即隐。
 *  实现约束：事件委托挂在容器上（一个监听器服务整列表），首悬抓
 *  /torrents/{id} 详情进模块级 Map 缓存（会话内不重抓）；触屏设备
 *  不弹（hover 语义不存在，matchMedia 检测）。RSC 页面零侵入——
 *  只需在列表容器旁挂本 island。 */

interface PreviewData {
  small_descr: string | null;
  size: number;
  seeders: number;
  leechers: number;
  times_completed: number;
  promotion: string | null;
  anonymous: boolean;
  owner_name: string | null;
}

const cache = new Map<number, PreviewData>();
const HOVER_DELAY_MS = 400;

export function TorrentHoverPreview({ dict: _d }: { dict?: unknown }) {
  const { dict } = useI18n();
  const t = dict.torrents;
  const [card, setCard] = useState<{
    x: number;
    y: number;
    id: number;
    data: PreviewData | null;
  } | null>(null);
  const timer = useRef<ReturnType<typeof setTimeout> | null>(null);
  const [touch, setTouch] = useState(false);

  useEffect(() => {
    setTouch(window.matchMedia("(hover: none)").matches);
  }, []);

  useEffect(() => {
    if (touch) return;

    const show = async (anchor: HTMLAnchorElement, id: number) => {
      const rect = anchor.getBoundingClientRect();
      let data = cache.get(id) ?? null;
      setCard({ x: rect.left, y: rect.bottom + 6, id, data });
      if (!data) {
        try {
          const r = await api.get<TorrentListItem>(
            `/api/v1/torrents/${id}`,
          );
          data = {
            small_descr: r.small_descr,
            size: r.size,
            seeders: r.seeders,
            leechers: r.leechers,
            times_completed: r.times_completed,
            promotion: (r.promotion as string | null) ?? null,
            anonymous: r.anonymous,
            owner_name: r.owner_name ?? null,
          };
          cache.set(id, data);
          setCard((c) => (c && c.id === id ? { ...c, data } : c));
        } catch {
          setCard(null);
        }
      }
    };

    const onOver = (e: Event) => {
      const a = (e.target as HTMLElement).closest?.(
        'a[href^="/torrent/"]',
      ) as HTMLAnchorElement | null;
      if (!a) return;
      const m = a.getAttribute("href")?.match(/^\/torrent\/(\d+)$/);
      if (!m) return;
      const id = Number(m[1]);
      if (timer.current) clearTimeout(timer.current);
      timer.current = setTimeout(() => show(a, id), HOVER_DELAY_MS);
    };
    const onOut = (e: Event) => {
      const a = (e.target as HTMLElement).closest?.(
        'a[href^="/torrent/"]',
      );
      if (!a) return;
      if (timer.current) clearTimeout(timer.current);
      setCard(null);
    };

    document.addEventListener("mouseover", onOver);
    document.addEventListener("mouseout", onOut);
    return () => {
      document.removeEventListener("mouseover", onOver);
      document.removeEventListener("mouseout", onOut);
      if (timer.current) clearTimeout(timer.current);
    };
  }, [touch]);

  if (touch || !card) return null;
  const d = card.data;
  // 视口右缘防溢出：卡片宽 300，出界则左移
  const x = Math.min(card.x, Math.max(8, window.innerWidth - 308));
  const promo =
    d?.promotion && dict.promotion[
      d.promotion as keyof typeof dict.promotion
    ];

  return (
    <div
      className="thov"
      style={{ left: x, top: card.y }}
      data-torrent-preview={card.id}
      onMouseLeave={() => setCard(null)}
    >
      {!d && <p className="thov__loading">{t.hoverLoading}</p>}
      {d && (
        <>
          {promo && (
            <p className="thov__promo">
              {dict.promotion[
                d.promotion as keyof typeof dict.promotion
              ]}
            </p>
          )}
          {d.small_descr && (
            <p className="thov__descr">{d.small_descr}</p>
          )}
          <p className="thov__stats">
            <span title={t.colSeeders}>🌱 {d.seeders}</span>
            <span title={t.colLeechers}>⬇️ {d.leechers}</span>
            <span title={t.colCompleted}>✅ {d.times_completed}</span>
            <span title={t.colSize}>{formatBytes(d.size)}</span>
          </p>
          <p className="thov__owner">
            {d.anonymous
              ? dict.torrent.anonymous
              : (d.owner_name ?? "—")}
          </p>
        </>
      )}
    </div>
  );
}
