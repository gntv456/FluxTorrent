"use client";

import Link from "next/link";
import { formatBytes, promotionBadge } from "@/lib/format";
import { useI18n } from "@/i18n/client";
import { PosterImage } from "@/components/poster-image";

/** /free 机器人回话下发的种子摘要（GET /shoutbox/bot/exec 的 cards[]） */
export type FreeCard = {
  id: number;
  name: string;
  small_descr?: string | null;
  size: number;
  seeders: number;
  leechers: number;
  poster?: string | null;
  promotion?: import("@fluxtorrent/domain-types").PromotionKind | null;
  /** P-551：促销「免费」只免上传量扣费，不免种子定价（charge_for_download
   *  从不看促销）；有价种进播报时把定价一起下发，形状同下载口的价格标 */
  price?: number;
};

/** 喊话盒「播报带种卡」（0420）：/free 回话把当前免费种子压成紧凑行卡
 *  ——与论坛 !torrent 引用卡同族，但按聊天室节奏收窄成单行（海报 + 名称
 *  + 促销标 + 大小/做种）。数据随回话下发，不另打种子接口（零新端点）。 */
export function ShoutFreeCards({ cards }: { cards: FreeCard[] }) {
  const { dict, currency } = useI18n();
  const dTorrent = dict.torrent;
  const dPromo = dict.promotion;
  return (
    <div className="mt-1 flex flex-col gap-1">
      {cards.map((c) => {
        const promo = promotionBadge(c.promotion ?? null);
        return (
          <Link
            key={c.id}
            href={`/torrent/${c.id}`}
            className={
              "flex items-center gap-2 rounded-[var(--r-sm)] border " +
              "border-line bg-[var(--surface-card)] p-1 " +
              "transition-colors hover:border-sky"
            }
          >
            <PosterImage
              src={c.poster ?? undefined}
              alt={c.name}
              size="sm"
              rounded="sm"
              className="shrink-0"
            />
            <span
              className="min-w-0 flex-1 truncate text-xs font-bold text-ink"
            >
              {c.name}
            </span>
            {promo && (
              <span
                className={
                  promo.key === "free"
                    ? "torrents-promo torrents-promo--free"
                    : "torrents-promo"
                }
              >
                {dPromo[promo.key]}
              </span>
            )}
            {(c.price ?? 0) > 0 && (
              <span className="td-dl-price" title={dTorrent.priceHint}>
                {c.price} {currency}
              </span>
            )}
            <span className="shrink-0 text-[11px] text-sub">
              {formatBytes(c.size)} · {dTorrent.seeding} {c.seeders}
            </span>
          </Link>
        );
      })}
    </div>
  );
}
