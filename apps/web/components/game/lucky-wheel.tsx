"use client";

import { useEffect, useRef, useState } from "react";

export interface WheelPrize {
  label: string;
  /** magic | item；物品位价值不在 payout 上 */
  kind?: "magic" | "item";
  value?: number;
  icon?: string;
  qty?: number;
  weight_permille: number;
  payout: number;
}

const SPIN_MS = 4000;
const VIEW = 320;
const CX = VIEW / 2;
const CY = VIEW / 2;
const R = 150;

/** 极坐标取点：0° 在正上方、顺时针为正 */
function polar(deg: number, r: number): [number, number] {
  const a = ((deg - 90) * Math.PI) / 180;
  return [CX + r * Math.cos(a), CY + r * Math.sin(a)];
}

/** 扇形路径（从圆心到弧）*/
function sector(a1: number, a2: number): string {
  const [x1, y1] = polar(a1, R);
  const [x2, y2] = polar(a2, R);
  const large = a2 - a1 > 180 ? 1 : 0;
  return `M ${CX} ${CY} L ${x1} ${y1} A ${R} ${R} 0 ${large} 1 ${x2} ${y2} Z`;
}

/** 档位配色：大奖金、高档紫、中档蓝、小档灰（与九宫格/扭蛋同语义） */
function segColor(v: number, tk: number): string {
  if (v >= tk * 10) return "#f0d9a8";
  if (v >= tk * 3) return "#c9d4f2";
  if (v >= tk) return "#c5dff0";
  return "#eef5fc";
}

/** 扇区读数：魔力档 ×N、物品档显示图标（与九宫格一致） */
function glyph(p: WheelPrize): string {
  if (p.kind === "item") {
    return p.qty && p.qty > 1 ? `${p.icon ?? "🎁"}×${p.qty}` : (p.icon ?? "🎁");
  }
  return p.payout === 0 ? "0" : `×${p.payout}`;
}

/**
 * 大转盘：指针固定在顶部，盘面顺时针旋转后停格。
 * 结果在响应返回后才开始旋转（九宫格同纪律）。
 */
export function LuckyWheel({
  prizes,
  ticket,
  resultIndex,
  busy,
  disabled,
  reduced,
  onDraw,
  onLanded,
  goLabel,
  spinningLabel,
}: {
  prizes: WheelPrize[];
  ticket: number;
  resultIndex: number | null;
  busy: boolean;
  disabled?: boolean;
  reduced: boolean;
  onDraw: () => void;
  onLanded?: () => void;
  goLabel: string;
  spinningLabel: string;
}) {
  const [rot, setRot] = useState(0);
  const timer = useRef<number | null>(null);
  const n = prizes.length;
  const seg = n ? 360 / n : 360;

  useEffect(
    () => () => {
      if (timer.current) window.clearTimeout(timer.current);
    },
    [],
  );

  useEffect(() => {
    if (resultIndex === null) return;
    if (reduced) {
      onLanded?.();
      return;
    }
    // 目标：让该扇区中心停在顶部指针（旋转角 ≡ -(扇区中心) mod 360）
    const center = resultIndex * seg + seg / 2;
    const desired = (360 - center) % 360;
    setRot((prev) => {
      const cur = ((prev % 360) + 360) % 360;
      const delta = (((desired - cur) % 360) + 360) % 360;
      return prev + 360 * 5 + delta;
    });
    timer.current = window.setTimeout(() => onLanded?.(), SPIN_MS);
    return () => {
      if (timer.current) window.clearTimeout(timer.current);
    };
    // eslint-disable-next-line react-hooks/exhaustive-deps
  }, [resultIndex, reduced]);

  return (
    <div className="lw-rig">
      <div className="lw-disc-wrap">
        <span className="lw-pointer" aria-hidden />
        <svg
          className={`lw-disc${reduced ? "" : " spin"}`}
          viewBox={`0 0 ${VIEW} ${VIEW}`}
          style={{ transform: `rotate(${rot}deg)` }}
          aria-hidden
        >
          <defs>
            <radialGradient id="lwRim" cx="0.5" cy="0.3" r="0.8">
              <stop offset="0%" stopColor="#e8c888" />
              <stop offset="100%" stopColor="#b08a4a" />
            </radialGradient>
            <radialGradient id="lwGo" cx="0.35" cy="0.3" r="0.8">
              <stop offset="0%" stopColor="#f7e6c0" />
              <stop offset="60%" stopColor="#d4af7a" />
              <stop offset="100%" stopColor="#b08a4a" />
            </radialGradient>
          </defs>
          {/* 金色渐变外圈 */}
          <circle cx={CX} cy={CY} r={R + 8} fill="url(#lwRim)" />
          <circle cx={CX} cy={CY} r={R + 3} fill="#fff" />
          {/* 灯珠：金色与白色交替，沿外圈均匀分布 */}
          {Array.from({ length: 12 }).map((_, i) => {
            const a = (i * 360) / 12;
            const [lx, ly] = polar(a, R + 5);
            const isGold = i % 2 === 0;
            return (
              <circle
                key={i}
                cx={lx}
                cy={ly}
                r={3.5}
                fill={isGold ? "#ffd700" : "#fff"}
                opacity={isGold ? 1 : 0.7}
              />
            );
          })}
          {prizes.map((p, i) => {
            const a1 = i * seg;
            const a2 = a1 + seg;
            const v = p.value ?? p.payout * ticket;
            const mid = (a1 + a2) / 2;
            const [tx, ty] = polar(mid, R * 0.66);
            return (
              <g key={i}>
                <path
                  d={sector(a1, a2)}
                  fill={segColor(v, ticket)}
                  stroke="#fff"
                  strokeWidth="2"
                />
                <text
                  x={tx}
                  y={ty}
                  textAnchor="middle"
                  dominantBaseline="middle"
                  fontSize={seg > 60 ? 15 : 13}
                  fontWeight="800"
                  fill={v >= ticket * 10 ? "#5a3d12" : "#fff"}
                  style={{ fontVariantNumeric: "tabular-nums" }}
                >
                  {glyph(p)}
                </text>
              </g>
            );
          })}
          <circle
            cx={CX}
            cy={CY}
            r={R}
            fill="none"
            stroke="rgba(255,255,255,0.6)"
            strokeWidth="2"
          />
        </svg>
        <button
          type="button"
          onClick={onDraw}
          disabled={busy || disabled}
          className="lw-hub"
          style={{
            background:
              "radial-gradient(circle at 35% 30%, #f7e6c0, " +
              "#d4af7a 60%, #b08a4a)",
            border: "3px solid #b08a4a",
            boxShadow:
              "0 4px 12px rgba(176,138,74,0.4), " +
              "inset 0 1px 2px rgba(255,255,255,0.5)",
          }}
          aria-label={`${goLabel} · ${ticket}`}
        >
          {busy ? spinningLabel : goLabel}
          {!busy && <small className="num">{ticket}</small>}
        </button>
      </div>
    </div>
  );
}
