"use client";

import { SwPrizeStrip } from "@/components/game/sw-enrich";
import {
  prizeIcon,
  prizeMeta,
  prizeTone,
  prizeValue,
} from "@/lib/wheel-prize";
import type { WheelPrize } from "@/lib/wheel-prize";

/**
 * 「奖池概率」标签页：顶部高价值 4 档 + 18 档完整概率网格。
 * 档位全部来自 `/games.wheel.prizes`（后端同一张池表），
 * 概率 = weight_permille / 10，不在前端另算权重。
 */
export function WheelPoolPanel({
  prizes,
  ticket,
  labels,
}: {
  prizes: WheelPrize[];
  ticket: number;
  labels: {
    picked: string;
    gridTitle: string;
    note: string;
    empty: string;
    metaItem: string;
    metaItemQty: string;
    metaMiss: string;
    metaMult: string;
  };
}) {
  if (prizes.length === 0) {
    return <p className="wp-pool-empty">{labels.empty}</p>;
  }
  const totalW = prizes.reduce((s, p) => s + p.weight_permille, 0) || 1;
  const pct = (p: WheelPrize) =>
    `${((p.weight_permille / totalW) * 100).toFixed(1)}%`;
  const top = [...prizes]
    .sort((a, b) => prizeValue(b, ticket) - prizeValue(a, ticket))
    .slice(0, 4)
    .map((p) => ({
      ic: prizeIcon(p, ticket),
      nm: p.label,
      odds: pct(p),
      tone:
        prizeTone(p, ticket) === "gold"
          ? ("gold" as const)
          : prizeTone(p, ticket) === "mint"
            ? ("lilac" as const)
            : ("sky" as const),
    }));

  return (
    <div className="wp-pool">
      <SwPrizeStrip label={labels.picked} items={top} />
      <section className="wp-pool-panel">
        <h2 className="wp-h2">{labels.gridTitle}</h2>
        <div className="wp-pool-grid">
          {prizes.map((p, i) => {
            const tone = prizeTone(p, ticket);
            return (
              <div key={i} className={`wp-pool-item ${tone}`}>
                <span className={`wp-pi-ic ${tone}`} aria-hidden>
                  {prizeIcon(p, ticket)}
                </span>
                <div className="wp-pi-mid">
                  <div className="wp-pi-nm">{p.label}</div>
                  <div className="wp-pi-meta">
                    {prizeMeta(p, ticket, {
                      item: labels.metaItem,
                      itemQty: labels.metaItemQty,
                      miss: labels.metaMiss,
                      mult: labels.metaMult,
                    })}
                  </div>
                </div>
                <span className="wp-pi-prob num">{pct(p)}</span>
              </div>
            );
          })}
        </div>
        <p className="wp-pool-note">{labels.note}</p>
      </section>
    </div>
  );
}
