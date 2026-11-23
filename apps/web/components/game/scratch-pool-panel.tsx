"use client";

import type { ScratchPrize } from "@/lib/games";

/**
 * 刮刮乐「奖池概率」标签页：完整档位概率网格 + 同源说明。
 * 档位全部来自 /games.scratch.prizes（后端同一张池表），概率按千分权重
 * 换算，不在前端另算权重；纯呈现哑件，文案由页面 i18n 传入。
 */
export function ScratchPoolPanel({
  prizes,
  labels,
}: {
  prizes: ScratchPrize[];
  labels: {
    gridTitle: string;
    note: string;
    empty: string;
    metaMiss: string;
  };
}) {
  if (prizes.length === 0) {
    return <p className="sp-pool-empty">{labels.empty}</p>;
  }
  // 后端未下发 rarity，按 payout 倍数分档配色：高倍金、中倍紫、其余蓝。
  const toneOf = (p: ScratchPrize) =>
    p.payout >= 10
      ? ("gold" as const)
      : p.payout >= 2
        ? ("lilac" as const)
        : ("sky" as const);

  return (
    <div className="sp-pool">
      <section className="sp-pool-panel">
        <h2 className="sp-h2">{labels.gridTitle}</h2>
        <div className="sp-pool-grid">
          {prizes.map((p, i) => {
            const tone = toneOf(p);
            const odds = `${(p.weight_permille / 10).toFixed(1)}%`;
            const meta =
              p.payout > 0 ? `${p.payout.toFixed(1)}x` : labels.metaMiss;
            return (
              <div key={i} className={`sp-pool-item ${tone}`}>
                <span className={`sp-pi-ic ${tone}`} aria-hidden>
                  {p.kind === "item" ? "🎁" : "💎"}
                </span>
                <div className="sp-pi-mid">
                  <div className="sp-pi-nm">{p.label}</div>
                  <div className="sp-pi-meta">{meta}</div>
                </div>
                <span className="sp-pi-prob num">{odds}</span>
              </div>
            );
          })}
        </div>
        <p className="sp-pool-note">{labels.note}</p>
      </section>
    </div>
  );
}
