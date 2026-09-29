"use client";

import { useEffect, useRef, useState } from "react";

/** 3×3 九宫格：外圈顺时针 8 格 = 8 个奖档，中心 = GO */
const RING = [0, 1, 2, 5, 8, 7, 6, 3];
const TOTAL_MS = 2400;

export interface JggPrize {
  label: string;
  /** magic | item；物品位的价值不在 payout 上 */
  kind?: "magic" | "item";
  /** 该档给玩家的魔力等值：魔力位=票价×倍数，物品位=anchor×件数 */
  value?: number;
  item_key?: string;
  qty?: number;
  weight_permille: number;
  payout: number;
}

/**
 * 九宫格抽奖灯阵。
 * 关键：跑马灯在**响应返回后**才启动（旧实现是请求发出就开始转，快网空转、慢网转完没结果）；
 * 奖池文案全部来自后端下发的 prizes（修「前端硬编码 100x 与后端不符」）。
 */
export function JggGrid({
  prizes,
  ticket,
  resultIndex,
  busy,
  disabled,
  reduced,
  onDraw,
  onLanded,
  goLabel,
  drawingLabel,
}: {
  prizes: JggPrize[];
  ticket: number;
  resultIndex: number | null;
  busy: boolean;
  disabled?: boolean;
  reduced: boolean;
  onDraw: () => void;
  onLanded?: () => void;
  goLabel: string;
  drawingLabel: string;
}) {
  const [lit, setLit] = useState<number | null>(null);
  const [landed, setLanded] = useState(false);
  const timer = useRef<number | null>(null);

  // 开抽瞬间清掉上一局的高亮/中奖态（否则新一局开始前上次的中奖格还亮着）
  useEffect(() => {
    if (busy) {
      setLanded(false);
      setLit(null);
    }
  }, [busy]);

  useEffect(() => {
    if (resultIndex === null) return;
    setLanded(false);
    if (reduced) {
      setLit(resultIndex);
      setLanded(true);
      onLanded?.();
      return;
    }
    let step = 0;
    const laps = prizes.length * 2;
    const target = laps + resultIndex;
    const tick = () => {
      setLit(step % prizes.length);
      step++;
      if (step > target) {
        setLanded(true);
        onLanded?.();
        return;
      }
      const remain = target - step;
      const delay =
        remain <= 1 ? 400 : remain <= 3 ? 240 : remain <= 6 ? 160 : 90;
      timer.current = window.setTimeout(tick, delay);
    };
    tick();
    return () => {
      if (timer.current) window.clearTimeout(timer.current);
    };
    // eslint-disable-next-line react-hooks/exhaustive-deps
  }, [resultIndex, reduced]);

  const cellBase =
    "relative flex aspect-square flex-col items-center justify-center rounded-[var(--r-sm)] border-2 px-1 text-center transition-all duration-200";

  return (
    <div className="flex w-full max-w-[340px] flex-col items-center gap-3">
      <div className="grid w-full grid-cols-3 gap-2" aria-busy={busy}>
        {Array.from({ length: 9 }, (_, gi) => {
          if (gi === 4) {
            return (
              <button
                key="go"
                type="button"
                onClick={onDraw}
                disabled={busy || disabled}
                aria-label={`${goLabel} · ${ticket}`}
                className="flex aspect-square items-center justify-center rounded-[var(--r-sm)] bg-coral text-base font-black text-white active:scale-[0.97] disabled:opacity-60"
              >
                {busy ? drawingLabel : goLabel}
              </button>
            );
          }
          const pi = RING.indexOf(gi);
          const p = prizes[pi];
          if (!p) return <span key={gi} className={cellBase} />;
          const isLit = lit === pi;
          const isWin = landed && resultIndex === pi;
          // 物品档按价值判高低：payout 恒为 0，直接判会把免考核卡画成最低档样式
          const jack = p.kind === "item" ? (p.value ?? 0) >= ticket * 10 : p.payout >= 10;
          return (
            <div
              key={gi}
              className={`${cellBase} ${
                isWin
                  ? "scale-[1.06] border-sun bg-sun-soft shadow-[0_8px_22px_rgba(255,201,60,.35)]"
                  : isLit
                    ? "border-sun bg-sun-soft"
                    : jack
                      ? "border-[var(--border-soft)] bg-[var(--surface-card)]"
                      : "border-line bg-[var(--surface-card)]"
              }`}
            >
              <span
                className={`num text-sm font-black ${jack ? "text-[var(--warning)]" : ""}`}
              >
                {p.kind === "item"
                  ? `🎁${p.qty && p.qty > 1 ? `×${p.qty}` : ""}`
                  : p.payout === 0
                    ? "—"
                    : p.payout === 1
                      ? "↺"
                      : `×${p.payout}`}
              </span>
              <span className="mt-0.5 text-[10px] leading-tight text-sub">
                {p.label}
              </span>
            </div>
          );
        })}
      </div>
    </div>
  );
}
