"use client";

import { useEffect, useState } from "react";
import { api } from "@/lib/api-client";
import { useI18n } from "@/i18n/client";
import { formatBytes } from "@/lib/format";

/**
 * 相关种子（2026-10-08 二轮实证缺口 P1）。
 *
 * 数据源 = GET /api/v1/torrents/{id}/related，后端按「同版本组 > 同标签交集 >
 * 同分类 + 标题 trgm 相似」三档打分（见 torrents/related.rs）。客户端懒加载，
 * 失败静默隐藏（推荐位属增益，不该阻断详情页）。
 *
 * 促销徽标复用 NexusPHP 语义色（free/x2 等），与列表页口径一致。
 */

interface RelatedItem {
  id: number;
  name: string;
  small_descr: string | null;
  category_id: number;
  size: number;
  seeders: number;
  leechers: number;
  promotion: string | null;
  poster: string | null;
  times_completed: number;
}

/** 促销徽标文案与配色（与列表页 pro_free/pro_2up 同名同义） */
function promoBadge(p: string): { label: string; cls: string } {
  const map: Record<string, { label: string; cls: string }> = {
    free: { label: "Free", cls: "bg-sun text-ink" },
    half: { label: "50%", cls: "bg-lime text-ink" },
    x2: { label: "×2", cls: "bg-rose text-white" },
    x2free: { label: "2×Free", cls: "bg-rose text-white" },
    x2half: { label: "2×50%", cls: "bg-rose text-white" },
    p30: { label: "-30%", cls: "bg-indigo text-white" },
  };
  return map[p] ?? { label: p, cls: "bg-ink text-white" };
}

export function TorrentRelated({ torrentId }: { torrentId: number }) {
  const { dict } = useI18n();
  const [items, setItems] = useState<RelatedItem[] | null>(null);

  useEffect(() => {
    let alive = true;
    api
      .get<{ items: RelatedItem[] }>(
        `/api/v1/torrents/${torrentId}/related`,
      )
      .then((r) => {
        if (alive) setItems(r.items ?? []);
      })
      .catch(() => {
        if (alive) setItems([]);
      });
    return () => {
      alive = false;
    };
  }, [torrentId]);

  // 加载中 / 无数据 / 失败：整段隐藏（推荐位不该占位）
  if (!items || items.length === 0) return null;

  return (
    <section className="td-related">
      <h3 className="td-related__title">{dict.tdetail.relatedTitle}</h3>
      <ul className="td-related__list">
        {items.map((r) => (
          <li key={r.id} className="td-related__item">
            <a href={`/torrent/${r.id}`} className="td-related__link">
              {r.promotion && (
                <span
                  className={`sticker ${promoBadge(r.promotion).cls}`}
                >
                  {promoBadge(r.promotion).label}
                </span>
              )}
              <span className="td-related__name" title={r.name}>
                {r.name}
              </span>
              <span className="td-related__meta num">
                <span className="td-related__size">
                  {formatBytes(r.size)}
                </span>
                <span className="td-related__seed">
                  🌱 {r.seeders}
                </span>
                <span className="td-related__leech">
                  ⬇️ {r.leechers}
                </span>
              </span>
            </a>
          </li>
        ))}
      </ul>
    </section>
  );
}
