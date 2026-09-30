"use client";

import { PANEL_LG } from "@/lib/ui-classes";
import { useI18n } from "@/i18n/client";
import { HistoryStrip } from "@/components/game/game-kit-feedback";

export interface PrizeChip {
  label: string;
  weight_permille: number;
  payout: number;
  value?: number;
}

/** 钓鱼侧栏：奖池档位 + 战绩。抽成组件是为了让专注页守住 300 行。 */
export function FishingSide({
  prizes,
  ticket,
  hist,
}: {
  prizes: PrizeChip[];
  ticket: number;
  hist: { net: number; game?: string }[];
}) {
  const { dict } = useI18n();
  const t = dict.games;
  const tf = t.fishing;
  return (
    <div className={PANEL_LG}>
      <h2 className="mb-2 font-display text-base">{tf.prizePool}</h2>
      <div className="flex flex-wrap gap-1.5">
        {prizes.map((p, i) => (
          <span
            key={i}
            className={`rounded-full px-2 py-0.5 text-[11px] font-bold ${
              (p.value ?? p.payout * ticket) >= ticket * 5
                ? "bg-sun-soft text-[var(--warning)]"
                : (p.value ?? p.payout * ticket) > ticket
                  ? "bg-mint-soft text-[var(--mint)]"
                  : "bg-[var(--surface-sunken)] text-sub"
            }`}
          >
            {p.label} {(p.weight_permille / 10).toFixed(1)}%
          </span>
        ))}
      </div>
      <p className="mt-2 text-[11px] text-sub">{tf.poolNote}</p>
      <h2 className="mb-2 mt-3 font-display text-base">{t.history}</h2>
      <HistoryStrip rounds={hist} />
    </div>
  );
}
