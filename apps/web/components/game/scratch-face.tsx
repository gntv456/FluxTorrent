"use client";

import { useI18n } from "@/i18n/client";

export interface ScratchOutcomeView {
  multiplier: number;
  payout: number;
  net: number;
  /** 展示用稀有度（1..5） */
  rarity: number;
  /** 物品位的目录图标 */
  icon?: string;
  /** 档位配图 URL（站长配）；有则优先于 icon */
  image_url?: string;
}

/**
 * 刮刮乐票面读数：稀有度星级 + 图标/配图 + 倍数 + 派彩。
 * 抽成组件是为了让专注页守住 300 行，也让「揭晓长什么样」只写一处。
 */
export function ScratchFace({
  phase,
  outcome,
}: {
  phase: "idle" | "buying" | "scratchable" | "done";
  outcome: ScratchOutcomeView | null;
}) {
  const { dict } = useI18n();
  const ts = dict.games.scratch;
  if (phase === "buying") {
    return <span className="text-sm text-sub">{ts.buying}</span>;
  }
  if (!outcome) {
    return <span className="num text-4xl font-black text-sub">??</span>;
  }
  const big = outcome.multiplier >= 2;
  return (
    <div className="flex flex-col items-center gap-0.5">
      {/* 稀有度星级（1..5）：揭晓那一刻最直观的「值不值」信号 */}
      <span className={`sc-stars r${outcome.rarity}`} aria-hidden>
        {"★".repeat(Math.max(1, outcome.rarity))}
      </span>
      {outcome.image_url ? (
        // 站长配了图就用图（外链：不送 referrer，懒加载）
        // eslint-disable-next-line @next/next/no-img-element
        <img
          src={outcome.image_url}
          alt=""
          loading="lazy"
          referrerPolicy="no-referrer"
          className="h-12 w-12 object-contain"
        />
      ) : outcome.icon ? (
        <span className="text-4xl leading-none" aria-hidden>
          {outcome.icon}
        </span>
      ) : null}
      <span
        className={`num font-display text-[38px] ${
          big
            ? "text-[var(--warning)]"
            : outcome.multiplier > 0
              ? "text-mint"
              : "text-sub"
        }`}
      >
        {outcome.multiplier === 0 ? "0" : `${outcome.multiplier}x`}
      </span>
      <span className="text-xs text-sub">
        {outcome.multiplier === 0
          ? ts.thanks
          : `${ts.payout} ${outcome.payout}`}
      </span>
    </div>
  );
}
