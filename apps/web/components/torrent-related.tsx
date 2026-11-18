"use client";

import { useEffect, useState } from "react";
import "@/app/styles/ux-related.css";
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
 * M6.4 对齐本地完善版样图 .td-rel__*：右栏卡片内竖排，46×62
 * 缩略位（有海报显示图，无海报渐变底 + 分类文字），名称两行截断
 * + 做种数；全断点同构。
 *
 * C2：追加同系列 / 同字幕组两个推荐区块。不新增后端请求——结论
 * 从已取的 related 列表在前端分组得出：同系列 = group_id（0069
 * 版本组）；同字幕组 = subtitle_group。related 端点尚未投影这两列，
 * 故两区块默认不渲染；后端补齐字段后零改动点亮。
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
  /** 聚合键（可选）：related 端点未投影时区块自动隐藏 */
  group_id?: number | null;
  subtitle_group?: string | null;
}

/** 同键聚簇，只保留 ≥2 条的桶——一条不构成「其他版本/其他作品」。 */
function clusters(
  items: RelatedItem[],
  keyOf: (r: RelatedItem) => string | null,
): RelatedItem[][] {
  const by = new Map<string, RelatedItem[]>();
  for (const r of items) {
    const key = keyOf(r);
    if (!key) continue;
    const bucket = by.get(key);
    if (bucket) bucket.push(r);
    else by.set(key, [r]);
  }
  return Array.from(by.values()).filter((b) => b.length > 1);
}

/** 单个推荐条目：仍是原生 a[href]，键盘 Tab/Enter 与旧区块完全一致。 */
function RelLink({
  r,
  catMap,
  seedLabel,
}: {
  r: RelatedItem;
  catMap: Record<number, string>;
  seedLabel: string;
}) {
  return (
    <a className="td-rel__item ux-rel__item" href={`/torrent/${r.id}`}>
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
            {seedLabel} {r.seeders}
          </span>
        </span>
      </span>
    </a>
  );
}

export function TorrentRelated({
  torrentId,
  catMap,
}: {
  torrentId: number;
  /** 分类显示名映射（page 侧按站点档案 categories 传入；可序列化
   *  prop，函数过不了 RSC 边界）——组件不持站型词表 */
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
  const ux = dict.uxOpt;
  const series = clusters(items, (r) =>
    r.group_id == null ? null : `group:${r.group_id}`,
  );
  const subtitle = clusters(items, (r) => r.subtitle_group?.trim() || null);

  return (
    <>
      <div className="td-side-card">
        <h3 className="td-rel__title">{d.relatedTitle}</h3>
        {items.map((r) => (
          <RelLink key={r.id} r={r} catMap={catMap} seedLabel={d.relSeed} />
        ))}
      </div>

      {series.length > 0 && (
        <section
          className="td-side-card ux-rel__block"
          aria-label={ux.relatedSeries}
        >
          <h3 className="td-rel__title">{ux.relatedSeries}</h3>
          {series.map((bucket, i) => (
            <div className="ux-rel__cluster" key={`series-${i}`}>
              {bucket.map((r) => (
                <RelLink
                  key={r.id}
                  r={r}
                  catMap={catMap}
                  seedLabel={d.relSeed}
                />
              ))}
            </div>
          ))}
        </section>
      )}

      {subtitle.length > 0 && (
        <section
          className="td-side-card ux-rel__block"
          aria-label={ux.relatedGroup}
        >
          <h3 className="td-rel__title">{ux.relatedGroup}</h3>
          {subtitle.map((bucket, i) => (
            <div className="ux-rel__cluster" key={`subtitle-${i}`}>
              <span className="ux-rel__key">{bucket[0].subtitle_group}</span>
              {bucket.map((r) => (
                <RelLink
                  key={r.id}
                  r={r}
                  catMap={catMap}
                  seedLabel={d.relSeed}
                />
              ))}
            </div>
          ))}
        </section>
      )}
    </>
  );
}
