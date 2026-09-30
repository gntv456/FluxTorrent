"use client";

import { useEffect, useRef, useState } from "react";

export interface CapsulePrize {
  label: string;
  /** magic | item；物品位价值不在 payout 上 */
  kind?: "magic" | "item";
  value?: number;
  icon?: string;
  qty?: number;
  weight_permille: number;
  payout: number;
}

/** 穹顶里装饰用的扭蛋：固定散点 + 配色，纯视觉、不出货 */
const DOME_EGGS: { x: number; y: number; c: string }[] = [
  { x: 24, y: 70, c: "#ff8a63" },
  { x: 60, y: 44, c: "#5aa9ff" },
  { x: 96, y: 66, c: "#5ad0a0" },
  { x: 130, y: 40, c: "#e0a13a" },
  { x: 152, y: 78, c: "#ef6ab0" },
  { x: 40, y: 92, c: "#a06bf0" },
  { x: 84, y: 92, c: "#7d8698" },
  { x: 118, y: 92, c: "#ff8a63" },
  { x: 12, y: 40, c: "#5ad0a0" },
];

/** 档位配色：大奖金、高档紫、中档蓝、小档灰（与九宫格同一套稀有度语义） */
function tierColor(v: number, tk: number): string {
  if (v >= tk * 10) return "#e9b94e";
  if (v >= tk * 3) return "#a06bf0";
  if (v >= tk) return "#5aa9ff";
  return "#8b93a6";
}

/** 蛋面高光 + 底色合成（抽成函数，避免超宽行） */
function eggBg(hi: string, c: string): string {
  return `radial-gradient(circle at 36% 28%, ${hi}, transparent 46%), ${c}`;
}

/**
 * 扭蛋机：投币 → 机身震动 → 蛋从出货口掉落 → 开壳揭晓。
 * 结果在**响应返回后**才播放（与九宫格同纪律：快网不空转、慢网不空转完没结果）。
 */
export function CapsuleMachine({
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
  prizes: CapsulePrize[];
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
  const [phase, setPhase] = useState<"idle" | "shake" | "drop" | "open">(
    "idle",
  );
  const timers = useRef<number[]>([]);

  const clear = () => {
    timers.current.forEach((t) => window.clearTimeout(t));
    timers.current = [];
  };

  useEffect(() => {
    if (busy) {
      clear();
      setPhase("idle");
    }
  }, [busy]);

  useEffect(() => {
    if (resultIndex === null) return;
    if (reduced) {
      onLanded?.();
      return;
    }
    setPhase("shake");
    timers.current = [
      window.setTimeout(() => setPhase("drop"), 520),
      window.setTimeout(() => setPhase("open"), 1240),
      window.setTimeout(() => onLanded?.(), 1760),
    ];
    return clear;
    // eslint-disable-next-line react-hooks/exhaustive-deps
  }, [resultIndex, reduced]);

  const picked = resultIndex !== null ? prizes[resultIndex] : undefined;
  const dropColor = picked
    ? tierColor(picked.value ?? picked.payout * ticket, ticket)
    : "#8b93a6";

  return (
    <div className="cap-rig">
      <button
        type="button"
        onClick={onDraw}
        disabled={busy || disabled}
        aria-label={`${goLabel} · ${ticket}`}
        className={`cap-machine${phase === "shake" ? " shake" : ""}${
          busy && !reduced && phase === "shake" ? " spin" : ""
        }`}
        style={{
          border: 0,
          background: "none",
          cursor: busy || disabled ? "not-allowed" : "pointer",
          padding: 0,
        }}
      >
        <div className="cap-dome">
          <div className="cap-eggs" aria-hidden>
            {DOME_EGGS.map((e, i) => (
              <span
                key={i}
                className="cap-egg"
                style={{
                  left: e.x,
                  top: e.y,
                  background: eggBg("#fff8", e.c),
                }}
              />
            ))}
          </div>
        </div>
        <div className="cap-base">
          <span className="cap-knob" aria-hidden />
          <span className="cap-mouth" aria-hidden />
          {(phase === "drop" || phase === "open") && (
            <span
              className={`cap-drop${phase === "open" ? " open" : " fall"}`}
              style={{
                background: eggBg("#fff9", dropColor),
              }}
              aria-hidden
            />
          )}
        </div>
      </button>
      <p className="mt-2 text-xs text-sub">
        {busy ? drawingLabel : goLabel}
        <span className="num"> · {ticket}</span>
      </p>
    </div>
  );
}
