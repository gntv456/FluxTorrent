"use client";

/**
 * 我的背包：奖池物品位发出来的东西，按发放账聚合回玩家眼前。
 *
 * 件数不在前端累加，也不另存一份「已用数」—— 一律由后端从 arcade_item_grants
 * 反推（第二份清单必然漂移）。这里只做呈现。
 */

import { useI18n } from "@/i18n/client";

export interface ArcadePackItem {
  key: string;
  name: string;
  icon: string;
  kind: string;
  anchor: number;
  anchor_src: string;
  enabled: boolean;
  qty: number;
  last_game: string | null;
  last_at: string | null;
}

export interface ArcadePackData {
  total: number;
  kinds: number;
  items: ArcadePackItem[];
}

const fmt = (n: number) => Math.round(n).toLocaleString("en-US");

export function ArcadeBackpack({ pack }: { pack: ArcadePackData }) {
  const { dict, currency } = useI18n();
  const t = dict.games.arcade;
  const titles = dict.games as unknown as Record<string, { title?: string }>;
  const gameName = (g: string | null) =>
    g ? (titles[g]?.title ?? g) : "";

  return (
    <section className="arc-sec">
      <h3>
        {t.packTitle}
        {pack.kinds > 0 && (
          <span className="arc-note">
            {" "}
            · {pack.kinds} / {pack.total}
          </span>
        )}
      </h3>
      <div className="arc-shelf">
        {pack.items.map((x) => (
          <div key={x.key} className="arc-goods">
            <div className="gc" aria-hidden>
              {x.icon}
            </div>
            <div className="gn">
              {x.name}
              {x.qty > 1 && <span className="qty">×{x.qty}</span>}
            </div>
            <div className="gv">
              {gameName(x.last_game)}
              {x.anchor > 0 && ` · ${fmt(x.anchor)} ${currency}`}
            </div>
            {!x.enabled && <span className="zero">{t.packOff}</span>}
          </div>
        ))}
        {pack.items.length === 0 && (
          <div className="arc-note">{t.packEmpty}</div>
        )}
      </div>
    </section>
  );
}
