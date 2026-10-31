"use client";

/**
 * 作品内的一条种子行（「宽松」形态）：七列表格行 + 可展开的版本信息。
 *
 * 行数据由服务端下发（TorrentListItem，作品聚合的种子行带 screenshots），
 * 展开不额外请求。展开块口径对齐 Colapt「宽松」的种子详情：
 * 版本说明（small_descr）→ 发布者/发布时间 → 简介截图（最多 4 张）→
 * 「查看详情」。无截图时只给文字部分，不留空相册。
 */

import { useState } from "react";
import Link from "next/link";
import type { TorrentListItem } from "@fluxtorrent/domain-types";
import {
  formatBytes,
  promoLeftCompact,
  promotionBadge,
} from "@/lib/format";
import { dateLocale } from "@/i18n/config";
import { useI18n } from "@/i18n/client";
import { TorrentActions } from "@/components/torrent-actions";
import { nameChips, promoCls } from "@/components/work-chips";

/** 展开相册最多几张（其余在种子详情页） */
const MAX_SHOTS = 4;

/** 只认 http(s) 外链：库里存的是发种时提取的 URL，脏数据不许进 <img> */
function shotUrls(t: TorrentListItem): string[] {
  const raw = t.screenshots ?? [];
  return raw
    .filter((u) => typeof u === "string" && /^https?:\/\//.test(u))
    .slice(0, MAX_SHOTS);
}

export function WorkSeedRow({
  t,
  now,
}: {
  t: TorrentListItem;
  /** 服务端渲染时刻：客户端用同一 now 复算倒计时，水合不错位 */
  now: number;
}) {
  const { dict, locale } = useI18n();
  const [open, setOpen] = useState(false);
  const promo = promotionBadge(t.promotion);
  const promoLeft = promoLeftCompact(t.promotion_ends_at, now);
  const chips = nameChips(t.name, dict.works.chipSubs);
  const date = new Date(t.created_at).toLocaleDateString(dateLocale(locale));
  const published = new Date(t.created_at).toLocaleString(
    dateLocale(locale),
    { dateStyle: "short", timeStyle: "short" },
  );
  const shots = shotUrls(t);
  return (
    <div className="wk-sitem" data-torrent-id={t.id}>
      <div className="wk-row">
        <button
          type="button"
          className="wk-chev"
          aria-expanded={open}
          aria-label={dict.works.moreInfo}
          title={dict.works.moreInfo}
          onClick={() => setOpen((v) => !v)}
        >
          {open ? "▾" : "▸"}
        </button>
        <div className="wk-name">
          <div className="wk-chips">
            {chips.map((c) => (
              <span key={c.text} className={`wk-chip ${c.cls}`}>
                {c.text}
              </span>
            ))}
            {promo && (
              <span className={`wk-chip ${promoCls(t.promotion)}`}>
                {dict.promotion[promo.key]}
                {promoLeft && (
                  <span className="wk-chip-left">{promoLeft}</span>
                )}
              </span>
            )}
            {t.official && (
              <span className="wk-chip wk-cweb">
                {dict.torrents.officialTag}
              </span>
            )}
          </div>
          <div className="wk-rtitle">
            <Link href={`/torrent/${t.id}`} title={t.name}>
              {t.name}
            </Link>
          </div>
        </div>
        <span className="wk-time">{date}</span>
        <span className="wk-size">{formatBytes(t.size)}</span>
        <span className={`wk-seed${t.seeders === 0 ? " is-zero" : ""}`}>
          {t.seeders}
        </span>
        <span className="wk-leech">{t.leechers}</span>
        <span className="wk-done">{t.times_completed}</span>
        <div className="wk-act">
          <TorrentActions torrentId={t.id} torrentName={t.name} />
        </div>
      </div>
      {open && (
        <div className="wk-seed-more">
          {t.small_descr && (
            <p className="wk-sm-descr">{t.small_descr}</p>
          )}
          <p className="wk-sm-meta">
            <span>
              <b>{dict.works.publisher}</b>{" "}
              {t.anonymous
                ? dict.torrent.anonymous
                : (t.owner_name ?? "—")}
            </span>
            <span>
              <b>{dict.works.publishedAt}</b> {published}
            </span>
          </p>
          {shots.length > 0 && (
            <div className="wk-shots">
              {shots.map((u) => (
                <a
                  key={u}
                  href={u}
                  target="_blank"
                  rel="noreferrer"
                  className="wk-shot"
                >
                  {/* eslint-disable-next-line @next/next/no-img-element */}
                  <img
                    src={u}
                    alt=""
                    loading="lazy"
                    decoding="async"
                    referrerPolicy="no-referrer"
                  />
                </a>
              ))}
            </div>
          )}
          <div className="wk-sm-link">
            <Link href={`/torrent/${t.id}`}>
              {dict.works.viewTorrent} →
            </Link>
          </div>
        </div>
      )}
    </div>
  );
}
