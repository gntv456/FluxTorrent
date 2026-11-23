"use client";

import { useEffect, useRef, useState } from "react";
import {
  SLOT_REEL,
  idleReel,
  symbolForLabel,
  type SlotSymbol,
} from "./slot-symbols";

export interface SlotPrize {
  label: string;
  /** magic | item；物品位价值不在 payout 上 */
  kind?: "magic" | "item";
  value?: number;
  icon?: string;
  qty?: number;
  weight_permille: number;
  payout: number;
  rarity?: number;
}

/**
 * 经典三滚轮老虎机：拉杆 → 三列高速轮换 → 逐列停格 → 中奖格高亮。
 * 结果在**响应返回后**才进入停格演出（与转盘/扭蛋同纪律：快网不空转、
 * 慢网不空转完没结果）。机身整体可点，底部 GO 钮是主行动口。
 */
export function SlotMachine({
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
  prizes: SlotPrize[];
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
  const [phase, setPhase] = useState<"idle" | "spin" | "settle">("idle");
  const [cols, setCols] = useState<SlotSymbol[]>(() =>
    idleReel(SLOT_REEL, 3),
  );
  const [settled, setSettled] = useState<boolean[]>([false, false, false]);
  const timers = useRef<number[]>([]);
  const spinTimer = useRef<number | null>(null);

  const clearTimers = () => {
    timers.current.forEach((t) => window.clearTimeout(t));
    timers.current = [];
    if (spinTimer.current !== null) {
      window.clearInterval(spinTimer.current);
      spinTimer.current = null;
    }
  };

  useEffect(() => clearTimers, []);

  // 忙态起步：回到空闲静态格，再开高速轮换
  useEffect(() => {
    if (!busy) return;
    clearTimers();
    setSettled([false, false, false]);
    setPhase("spin");
    if (reduced) return;
    spinTimer.current = window.setInterval(() => {
      setCols((prev) =>
        prev.map(() => {
          const r = SLOT_REEL[Math.floor(Math.random() * SLOT_REEL.length)];
          return r;
        }),
      );
    }, 90);
  }, [busy, reduced]);

  useEffect(() => {
    if (resultIndex === null) return;
    const prize = prizes[resultIndex];
    const hit = symbolForLabel(SLOT_REEL, prize?.label, resultIndex);
    const won = (prize?.value ?? prize?.payout ?? 0) > ticket;
    // 中奖：三列同结果格（经典三同格观感）；未中：结果格居中，两侧回落邻格
    const ring = SLOT_REEL;
    const order = won
      ? [hit, hit, hit]
      : [
          ring[(ring.indexOf(hit) + 2) % ring.length],
          hit,
          ring[(ring.indexOf(hit) + 1) % ring.length],
        ];

    if (reduced) {
      setCols(order);
      setSettled([true, true, true]);
      setPhase("settle");
      onLanded?.();
      return;
    }

    // 逐列停格：左 → 中 → 右，每列 280ms；最后一列落定后回调
    setPhase("settle");
    if (spinTimer.current !== null) {
      window.clearInterval(spinTimer.current);
      spinTimer.current = null;
    }
    order.forEach((sym, i) => {
      timers.current.push(
        window.setTimeout(() => {
          setCols((prev) => prev.map((c, k) => (k === i ? sym : c)));
          setSettled((prev) => prev.map((s, k) => (k === i ? true : s)));
        }, 280 * (i + 1)),
      );
    });
    timers.current.push(
      window.setTimeout(() => onLanded?.(), 280 * 3 + 160),
    );
    // eslint-disable-next-line react-hooks/exhaustive-deps
  }, [resultIndex, reduced]);

  const jackpot =
    resultIndex !== null &&
    (prizes[resultIndex]?.value ?? prizes[resultIndex]?.payout ?? 0) >=
      ticket * 10;

  return (
    <div className="slot-rig">
      <button
        type="button"
        onClick={onDraw}
        disabled={busy || disabled}
        aria-label={`${goLabel} · ${ticket}`}
        className={`slot-body${phase === "spin" ? " spinning" : ""}${
          jackpot ? " jackpot" : ""
        }`}
        style={{ cursor: busy || disabled ? "not-allowed" : "pointer" }}
      >
        <div className="slot-top" aria-hidden>
          <span className="slot-lamp" />
          <span className="slot-title">LUCKY 777</span>
          <span className="slot-lamp" />
        </div>
        <div className="slot-reels">
          {cols.map((sym, i) => (
            <div
              key={i}
              className={`slot-reel${settled[i] ? " stopped" : ""}${
                sym.id === "seven" && settled[i] ? " hot" : ""
              }`}
            >
              {sym.glyph ? (
                <span className="slot-glyph" style={{ color: sym.tone }}>
                  {sym.glyph}
                </span>
              ) : (
                <span className="slot-icon">{sym.icon}</span>
              )}
            </div>
          ))}
        </div>
        <div className="slot-panel" aria-hidden>
          <span className="slot-needle" />
          <span className="slot-dial">
            {busy ? drawingLabel : goLabel}
          </span>
        </div>
      </button>
      <p className="mt-2 text-xs text-sub">
        {busy ? drawingLabel : goLabel}
        <span className="num"> · {ticket}</span>
      </p>
    </div>
  );
}
