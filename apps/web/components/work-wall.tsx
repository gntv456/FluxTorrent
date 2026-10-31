"use client";

/**
 * 作品聚合「瀑布」形态（Colapt 瀑布口径）：整面海报墙，点封面就地展开
 * 该作品的版本面板（前 8 条种子 + 「其余 N 个版本」→ 作品详情页）。
 *
 * 数据由服务端预投影成 WorkWallItem（只带墙与面板要用的列），点击不再
 * 请求接口；面板插在所选作品之后，墙用 `grid-auto-flow: dense` 把同一行
 * 补满，展开时不会在当前行留下空洞。
 */

import { Fragment, useState } from "react";
import Link from "next/link";
import { PosterImage } from "@/components/poster-image";
import { WorkSeedMini, type MiniSeed } from "@/components/work-seed-mini";
import { useI18n } from "@/i18n/client";
import { promoLeftCompact } from "@/lib/format";

export interface WallItem {
  key: string;
  title: string;
  poster: string | null;
  year: number | null;
  cat: string;
  /** 画质徽标（4K / 1080p），作品内没有就退回分类角标 */
  quality: string | null;
  /** 进行中的促销（Free 62D 之类），画在海报右上 */
  promo: string | null;
  /** 该促销的最早截止时刻（ISO；无截止 = null） */
  promoEnds: string | null;
  ratings: { label: string; value: string }[];
  torrents: number;
  seeders: number;
  latest: string;
  seeds: MiniSeed[];
}

export function WorkWall({
  items,
  now,
}: {
  items: WallItem[];
  /** 服务端渲染时刻：倒计时基准，客户端原样复算 */
  now: number;
}) {
  const { dict, locale } = useI18n();
  const [open, setOpen] = useState<string | null>(null);
  return (
    <ul className="wk-wall">
      {items.map((w) => {
        const href = `/works/${w.key}`;
        const on = open === w.key;
        const meta = [
          w.cat,
          w.year ? String(w.year) : null,
          w.ratings[0]
            ? `★ ${w.ratings[0].label} ${w.ratings[0].value}`
            : null,
        ]
          .filter(Boolean)
          .join(" · ");
        // 面板按 Colapt 口径：有官方版本就只列官方的，其余计数进「其余」链接
        const officials = w.seeds.filter((s) => s.official);
        const listed = officials.length > 0 ? officials : w.seeds;
        const rest = Math.max(0, w.torrents - listed.length);
        const promoLeft = promoLeftCompact(w.promoEnds, now);
        return (
          <Fragment key={w.key}>
            <li className="wk-wall-it" data-work-key={w.key}>
              <button
                type="button"
                className={`wk-wall-a${on ? " is-open" : ""}`}
                aria-expanded={on}
                title={w.title}
                onClick={() => setOpen(on ? null : w.key)}
              >
                <span className="wk-wall-poster">
                  <PosterImage
                    src={w.poster}
                    alt=""
                    size="lg"
                    className="h-full w-full object-cover"
                    sizes="(max-width: 640px) 45vw, 180px"
                    fallback={null}
                  />
                  <span className="wk-eyebrow">
                    {w.quality ?? w.cat}
                  </span>
                  {w.promo && (
                    <span className="wk-wall-promo">
                      {w.promo}
                      {promoLeft && (
                        <span className="wk-chip-left">{promoLeft}</span>
                      )}
                    </span>
                  )}
                </span>
                <span className="wk-wall-bar">
                  <span className="wk-wall-t">{w.title}</span>
                  <span className="wk-wall-m">{meta}</span>
                  <span className="wk-wall-n num">
                    {dict.works.seedsInWork.replace(
                      "{n}",
                      String(w.torrents),
                    )}
                    {" · ↑ "}
                    {w.seeders}
                  </span>
                </span>
              </button>
            </li>
            {on && (
              <li className="wk-wall-panel" data-work-key={w.key}>
                <div className="wk-wp-head">
                  <span className="wk-wp-poster">
                    <PosterImage
                      src={w.poster}
                      alt=""
                      size="sm"
                      className="h-full w-full object-cover"
                      fallback={null}
                    />
                  </span>
                  <span className="wk-wp-info">
                    <Link href={href} className="wk-wp-title">
                      {w.title}
                    </Link>
                    <span className="wk-wp-meta">
                      {[
                        w.cat,
                        w.year ? String(w.year) : null,
                        w.ratings
                          .map((r) => `★ ${r.label} ${r.value}`)
                          .join(" "),
                        dict.works.latestAt.replace("{t}", w.latest),
                      ]
                        .filter(Boolean)
                        .join(" · ")}
                    </span>
                  </span>
                  <Link href={href} className="wk-wp-open">
                    {dict.works.openWork} ›
                  </Link>
                  <button
                    type="button"
                    className="wk-wp-close"
                    aria-label={dict.works.previewClose}
                    onClick={() => setOpen(null)}
                  >
                    ×
                  </button>
                </div>
                {officials.length > 0 && (
                  <div className="wk-wp-group">
                    {dict.works.officialVersions}
                    <span className="num">{officials.length}</span>
                  </div>
                )}
                <WorkSeedMini
                  seeds={listed}
                  dict={dict}
                  locale={locale}
                  now={now}
                />
                {rest > 0 && (
                  <div className="wk-more">
                    <Link href={href}>
                      {dict.works.moreVersions.replace(
                        "{n}",
                        String(rest),
                      )}{" "}
                      ›
                    </Link>
                  </div>
                )}
              </li>
            )}
          </Fragment>
        );
      })}
    </ul>
  );
}
