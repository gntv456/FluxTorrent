"use client";

import { useEffect } from "react";
import { useI18n } from "@/i18n/client";

/** 娱乐屋反馈件（从 game-kit.tsx 按域拆出）：
 *  结果飘字 / 轻量浮层提示 / 战绩条（外壳与余额条留在 game-kit.tsx）。 */

/** 结果飘字：赢/平/输/大奖 四态，颜色之外另有文案与图标（色觉无障碍） */
export function ResultFlash({
  kind,
  text,
}: {
  kind: "win" | "lose" | "tie" | "jackpot" | null;
  text: string | null;
}) {
  const cls =
    kind === "jackpot"
      ? "text-[var(--warning)]"
      : kind === "win"
        ? "text-mint"
        : kind === "lose"
          ? "text-sub"
          : "text-ink";
  return (
    <p
      role="status"
      aria-live="polite"
      className={`flex min-h-[36px] items-center justify-center gap-1.5 text-center text-base font-black ${cls}`}
    >
      {text && <span className="animate-[fly_.28s_ease-out]">{text}</span>}
    </p>
  );
}

/** 轻量浮层提示：固定视口下方居中，动作反馈不会因为页面长而跑出视野。
 *  `key` 变化即重播进入动画；`role=status` + `aria-live` 保证读屏可闻。 */
export function GameToast({
  message,
  onDone,
  durationMs = 3200,
}: {
  message: { kind: "win" | "lose" | "tie" | "jackpot"; text: string } | null;
  onDone: () => void;
  durationMs?: number;
}) {
  useEffect(() => {
    if (!message) return;
    const id = window.setTimeout(onDone, durationMs);
    return () => window.clearTimeout(id);
  }, [message, onDone, durationMs]);

  if (!message) return null;
  const tone =
    message.kind === "jackpot"
      ? "bg-sun text-ink"
      : message.kind === "win"
        ? "bg-mint text-white"
        : message.kind === "tie"
          ? "bg-[var(--surface-card)] text-ink border border-[var(--border-deep)]"
          : "bg-[var(--surface-sunken)] text-sub border border-line";
  return (
    <div
      role="status"
      aria-live="polite"
      className="pointer-events-none fixed inset-x-0 bottom-6 z-50 flex justify-center px-4"
    >
      <p
        key={message.text}
        className={`animate-[fly_.24s_ease-out] max-w-[92vw] rounded-full px-4 py-2.5 text-sm font-bold shadow-[var(--shadow-hover)] ${tone}`}
      >
        {message.text}
      </p>
    </div>
  );
}

/** 战绩条：**按局**渲染（`GET /games/rounds`，一局一条 net）。
 *  直接用流水会一半负一半正、局数还翻倍，看起来像「输多赢少」——那是流水不是战绩。 */
export function HistoryStrip({
  rounds,
  empty,
}: {
  rounds: { net: number; game?: string }[];
  empty?: string;
}) {
  const { dict } = useI18n();
  if (rounds.length === 0) {
    return (
      <p className="py-2 text-center text-xs text-sub">
        {empty ?? dict.games.historyEmpty}
      </p>
    );
  }
  return (
    <div className="flex flex-wrap items-center gap-1.5">
      {rounds.map((r, i) => {
        const big = r.net >= 500;
        const win = r.net > 0;
        const tie = r.net === 0;
        return (
          <span
            key={i}
            title={`${r.game ?? "game"} ${r.net > 0 ? "+" : ""}${r.net}`}
            aria-label={`${r.net > 0 ? "+" : ""}${r.net}`}
            className={`inline-block h-4 w-4 rounded-full ${
              big
                ? "bg-sun"
                : win
                  ? "bg-mint"
                  : tie
                    ? "bg-[var(--surface-sunken)] ring-1 ring-[var(--border-deep)]"
                    : "border border-[var(--border-deep)] bg-[var(--surface-card)]"
            }`}
          />
        );
      })}
    </div>
  );
}
