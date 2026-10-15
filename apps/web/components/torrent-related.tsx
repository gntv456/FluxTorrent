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
 * M6.4 对齐本地完善版样图 .td-rel__*：右栏卡片内竖排，46×62 缩略位
 * （有海报显示图，无海报渐变底 + 分类文字），名称两行截断 + 做种数；
 * 全断点同构（弃 M6.3 的移动端横滑与促销徽标）。
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

export function TorrentRelated({
  torrentId,
  catMap,
}: {
  torrentId: number;
  /** 分类显示名映射（page 侧按站点档案 categories 传入；可序列化 prop，
   *  函数过不了 RSC 边界）——组件不持站型词表 */
  catMap: Record<number, string>;
}) {
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

  const d = dict.tdetail;
  return (
    <div className="td-side-card">
      <h3 className="td-rel__title">{d.relatedTitle}</h3>
      {items.map((r) => (
        <a key={r.id} className="td-rel__item" href={`/torrent/${r.id}`}>
          <span className="td-rel__thumb">
            {r.poster ? (
              <img src={r.poster} alt="" loading="lazy" />
            ) : (
              catMap[r.category_id] ?? `#${r.category_id}`
            )}
          </span>
          <span className="td-rel__body">
            <span className="td-rel__name" title={r.name}>
              {r.name}
            </span>
            <span className="td-rel__meta num">
              {formatBytes(r.size)} ·{" "}
              <span className="td-rel__seed">
                {d.relSeed} {r.seeders}
              </span>
            </span>
          </span>
        </a>
      ))}
    </div>
  );
}
