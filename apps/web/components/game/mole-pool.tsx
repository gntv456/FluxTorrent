"use client";

import { SwPrizeStrip } from "@/components/game/sw-enrich";
import { moleCeiling, moleIcon, moleValue } from "@/lib/mole-tier";
import type { MolePrize } from "@/lib/mole-tier";
import { trimMult } from "@/lib/wheel-prize";

/**
 * 「奖池概率」标签：顶部高价值 4 档 + 逐档概率网格。
 * 概率 = weight_permille / 10（按**每次冒头**计，不是按局）；
 * 底部单独一行炸弹：它不进池子，是排程里固定 2 只的扣分项。
 */
export function MolePoolPanel({
  prizes,
  ticket,
  spawns,
  bombSpawns,
  bombPenalty,
  labels,
}: {
  prizes: MolePrize[];
  ticket: number;
  spawns: number;
  bombSpawns: number;
  bombPenalty: number;
  labels: {
    picked: string;
    gridTitle: string;
    note: string;
    empty: string;
    metaMiss: string;
    metaMult: string;
    metaPenalty: string;
    bombName: string;
    bombNote: string;
    ceiling: string;
  };
}) {
  if (prizes.length === 0) {
    return <p className="wp-pool-empty">{labels.empty}</p>;
  }
  const totalW = prizes.reduce((s, p) => s + p.weight_permille, 0) || 1;
  const pct = (p: MolePrize) =>
    `${((p.weight_permille / totalW) * 100).toFixed(1)}%`;
  const top = [...prizes]
    .sort((a, b) => moleValue(b, ticket) - moleValue(a, ticket))
    .slice(0, 4)
    .map((p) => ({
      ic: moleIcon(moleValue(p, ticket), ticket),
      nm: p.label,
      odds: pct(p),
      tone:
        moleValue(p, ticket) >= ticket * 3
          ? ("gold" as const)
          : moleValue(p, ticket) > 0
            ? ("lilac" as const)
            : ("sky" as const),
    }));
  const bombPct = spawns + bombSpawns > 0
    ? ((bombSpawns / (spawns + bombSpawns)) * 100).toFixed(1)
    : "0";
  const ceiling = moleCeiling(prizes, ticket, spawns);

  return (
    <div className="wp-pool">
      <SwPrizeStrip label={labels.picked} items={top} />
      <section className="wp-pool-panel">
        <h2 className="wp-h2">{labels.gridTitle}</h2>
        <div className="wp-pool-grid">
          <div className="wp-pool-item bomb">
            <span className="wp-pi-ic bomb" aria-hidden>
              💣
            </span>
            <div className="wp-pi-mid">
              <div className="wp-pi-nm">{labels.bombName}</div>
              <div className="wp-pi-meta">
                {labels.metaPenalty.replace("{n}", String(bombPenalty))}
              </div>
            </div>
            <span className="wp-pi-prob num">{bombPct}%</span>
          </div>
          {prizes.map((p, i) => {
            const v = moleValue(p, ticket);
            return (
              <div
                key={i}
                className={`wp-pool-item ${
                  v >= ticket * 3 ? "gold" : v > 0 ? "mint" : "mist"
                }`}
              >
                <span className="wp-pi-ic" aria-hidden>
                  {moleIcon(v, ticket)}
                </span>
                <div className="wp-pi-mid">
                  <div className="wp-pi-nm">{p.label}</div>
                  <div className="wp-pi-meta">
                    {p.payout === 0
                      ? labels.metaMiss
                      : `${labels.metaMult.replace(
                          "{n}",
                          trimMult(p.payout),
                        )} · ${v}`}
                  </div>
                </div>
                <span className="wp-pi-prob num">{pct(p)}</span>
              </div>
            );
          })}
        </div>
        <p className="wp-pool-note">{labels.note}</p>
        <p className="wp-pool-note">
          {labels.bombNote.replace("{n}", String(bombSpawns))}
        </p>
        <p className="wp-pool-note">
          {labels.ceiling.replace("{n}", String(ceiling))}
        </p>
      </section>
    </div>
  );
}
