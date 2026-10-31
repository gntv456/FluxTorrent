/**
 * 作品聚合里的一条紧凑种子行（「网格」卡与「瀑布」预览面板共用）：
 * chips / 片名 / 时间 · 体积 · 做种 · 下载 / 操作。
 *
 * 列口径与 WorkSeedRows 同源，但只要求行里有这几列——「瀑布」面板传的是
 * 瘦身投影（不带 tags/sec_names/info_hash），省客户端载荷。
 * 无 hooks；图标与操作列沿用 TorrentActions。
 */

import Link from "next/link";
import type { PromotionKind } from "@fluxtorrent/domain-types";
import {
  formatBytes,
  promoLeftCompact,
  promotionBadge,
} from "@/lib/format";
import { dateLocale, type Locale } from "@/i18n/config";
import { TorrentActions } from "@/components/torrent-actions";
import { nameChips, promoCls } from "@/components/work-chips";
import type { Dict } from "@/i18n/zh-CN";

/** 迷你行所需的最小列集（作品接口的种子行与瀑布瘦身投影都满足） */
export interface MiniSeed {
  id: number;
  name: string;
  size: number;
  seeders: number;
  leechers: number;
  created_at: string;
  promotion: PromotionKind | null;
  /** 促销截止时刻（ISO；无截止 = null）——倒计时用 */
  promotion_ends_at: string | null;
  official: boolean;
}

export function WorkSeedMini({
  seeds,
  dict,
  locale,
  now,
}: {
  seeds: MiniSeed[];
  dict: Dict;
  locale: Locale;
  /** 服务端渲染时刻：倒计时以它为基准，服务端/客户端同值 */
  now: number;
}) {
  const t = dict.torrents;
  return (
    <ul className="wk-mini">
      {seeds.map((s) => {
        const promo = promotionBadge(s.promotion);
        const promoLeft = promoLeftCompact(s.promotion_ends_at, now);
        const chips = nameChips(s.name, dict.works.chipSubs);
        const date = new Date(s.created_at).toLocaleDateString(
          dateLocale(locale),
        );
        return (
          <li key={s.id} className="wk-mrow" data-torrent-id={s.id}>
            <div className="wk-mrow-main">
              <div className="wk-chips">
                {chips.map((c) => (
                  <span key={c.text} className={`wk-chip ${c.cls}`}>
                    {c.text}
                  </span>
                ))}
                {promo && (
                  <span className={`wk-chip ${promoCls(s.promotion)}`}>
                    {dict.promotion[promo.key]}
                    {promoLeft && (
                      <span className="wk-chip-left">{promoLeft}</span>
                    )}
                  </span>
                )}
                {s.official && (
                  <span className="wk-chip wk-cweb">
                    {dict.torrents.officialTag}
                  </span>
                )}
              </div>
              <div className="wk-rtitle">
                <Link href={`/torrent/${s.id}`} title={s.name}>
                  {s.name}
                </Link>
              </div>
              <div className="wk-mrow-meta">
                <span>{date}</span>
                <span>{formatBytes(s.size)}</span>
                <span title={t.colSeeders}>↑ {s.seeders}</span>
                <span title={t.colLeechers}>↓ {s.leechers}</span>
              </div>
            </div>
            <div className="wk-act">
              <TorrentActions torrentId={s.id} torrentName={s.name} />
            </div>
          </li>
        );
      })}
    </ul>
  );
}
