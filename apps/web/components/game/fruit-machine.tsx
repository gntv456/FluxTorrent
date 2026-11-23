"use client";

import { useEffect, useRef, useState } from "react";
import {
  FRUIT_REEL,
  symbolForLabel,
  type SlotSymbol,
} from "./slot-symbols";
import type { SlotPrize } from "./slot-machine";

/**
 * 水果机（参考图街机面板）：顶部灯排 + 中央蓝幕舞台 + 左右倍率装饰列 +
 * 底部符号赔率牌。后端仍是池抽一档，面板不做「选符号下注」——底部牌只公示
 * 各符号对应的档位倍数，主行动口在页内 CTA 行。
 * 结果在响应返回后才落灯演出（与三滚轮同纪律）。
 */
export function FruitMachine({
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
  title,
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
  title: string;
}) {
  const [phase, setPhase] = useState<"idle" | "spin" | "settle">("idle");
  const [lit, setLit] = useState(0);
  const [result, setResult] = useState<SlotSymbol | null>(null);
  const timers = useRef<number[]>([]);
  const lampTimer = useRef<number | null>(null);

  const clearTimers = () => {
    timers.current.forEach((t) => window.clearTimeout(t));
    timers.current = [];
    if (lampTimer.current !== null) {
      window.clearInterval(lampTimer.current);
      lampTimer.current = null;
    }
  };

  useEffect(() => clearTimers, []);

  // 忙态：灯排循环跑马灯
  useEffect(() => {
    if (!busy) return;
    clearTimers();
    setResult(null);
    setPhase("spin");
    if (reduced) return;
    lampTimer.current = window.setInterval(() => {
      setLit((v) => (v + 1) % FRUIT_REEL.length);
    }, 160);
  }, [busy, reduced]);

  useEffect(() => {
    if (resultIndex === null) return;
    const prize = prizes[resultIndex];
    const sym = symbolForLabel(FRUIT_REEL, prize?.label, resultIndex);
    if (reduced) {
      setResult(sym);
      setPhase("settle");
      onLanded?.();
      return;
    }
    setPhase("settle");
    if (lampTimer.current !== null) {
      window.clearInterval(lampTimer.current);
      lampTimer.current = null;
    }
    // 中央灯窗先闪两下再落定符号
    timers.current.push(
      window.setTimeout(() => setLit(FRUIT_REEL.indexOf(sym)), 180),
    );
    timers.current.push(
      window.setTimeout(() => setResult(sym), 420),
    );
    timers.current.push(
      window.setTimeout(() => onLanded?.(), 900),
    );
    // eslint-disable-next-line react-hooks/exhaustive-deps
  }, [resultIndex, reduced]);

  const hitPrize = resultIndex !== null ? prizes[resultIndex] : undefined;
  const mult = hitPrize
    ? ((hitPrize.value ?? hitPrize.payout * ticket) / ticket) || 0
    : 0;

  return (
    <div className="fruit-rig">
      <button
        type="button"
        onClick={onDraw}
        disabled={busy || disabled}
        aria-label={`${goLabel} · ${ticket}`}
        className={`fruit-body${phase === "spin" ? " running" : ""}`}
        style={{ cursor: busy || disabled ? "not-allowed" : "pointer" }}
      >
        {/* 顶部灯排 */}
        <div className="fruit-lights" aria-hidden>
          {FRUIT_REEL.map((s, i) => (
            <span
              key={s.id}
              className={`fruit-light${
              lit === i && phase !== "settle" ? " on" : ""
            }${result?.id === s.id ? " hit" : ""}`}
            >
              {s.glyph ?? s.icon}
            </span>
          ))}
        </div>

        <div className="fruit-stage">
          {/* 左右倍率装饰列（宽屏才显） */}
          <div className="fruit-side left" aria-hidden>
            <span className="fruit-chip">×2</span>
            <span className="fruit-chip luck">LUCK!</span>
            <span className="fruit-chip">×2</span>
          </div>

          <div className="fruit-center">
            <div className="fruit-heading">{title}</div>
            <div className="fruit-window">
              {result ? (
                <>
                  <span className="fruit-icon">{result.icon}</span>
                  <span className="fruit-label">{hitPrize?.label}</span>
                </>
              ) : (
                <span className="fruit-placeholder">
                  {busy ? drawingLabel : "?"}
                </span>
              )}
            </div>
            <div className="fruit-led" aria-hidden>
              <span className="num">
                {phase === "settle" && result
                  ? `×${mult.toFixed(mult % 1 ? 1 : 0)}`
                  : "000"}
              </span>
            </div>
          </div>

          <div className="fruit-side right" aria-hidden>
            <span className="fruit-chip">×2</span>
            <span className="fruit-chip luck">GOOD</span>
            <span className="fruit-chip">×2</span>
          </div>
        </div>

        {/* 底部符号赔率牌：档位 → 符号映射公示 */}
        <div className="fruit-pays" aria-hidden>
          {prizes.slice(0, 8).map((p, i) => {
            const sym = symbolForLabel(FRUIT_REEL, p.label, i);
            const m = (p.value ?? p.payout * ticket) / (ticket || 1);
            return (
              <span key={i} className="fruit-pay">
                <span className="fruit-pay-ic">
                  {sym.glyph ?? sym.icon}
                </span>
                <span className="num fruit-pay-mult">
                  {m > 0 ? `×${m.toFixed(m % 1 ? 1 : 0)}` : "—"}
                </span>
              </span>
            );
          })}
        </div>
      </button>
      <p className="mt-2 text-xs text-sub">
        {busy ? drawingLabel : goLabel}
        <span className="num"> · {ticket}</span>
      </p>
    </div>
  );
}
