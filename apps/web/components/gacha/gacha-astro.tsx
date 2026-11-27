"use client";

/**
 * /gacha 星盘保底环 + 规则行/点阵小件（2026-11-27 从 gacha-stage.tsx 拆出
 * 守行数门禁）：Astro 演出环（星空背景 SKY 固定种子 SSR 一致）与
 * RuleStep/DotMatrix 纯展示小件。
 */
import type { ReactNode } from "react";

/* ---------- 星空背景（固定种子，SSR 与客户端一致） ---------- */
export const SKY = (() => {
  let seed = 20261015;
  const rnd = () => {
    seed = (seed * 1103515245 + 12345) & 0x7fffffff;
    return seed / 0x7fffffff;
  };
  const stars = Array.from({ length: 130 }, (_, i) => ({
    key: i,
    cx: +(rnd() * 1200).toFixed(1),
    cy: +(rnd() * 760).toFixed(1),
    r: rnd() < 0.1 ? 1.5 : 0.7,
    o: +(0.18 + rnd() * 0.55).toFixed(2),
  }));
  const groups = [
    [
      [120, 140],
      [196, 96],
      [268, 148],
      [246, 226],
      [158, 214],
    ],
    [
      [820, 120],
      [900, 178],
      [982, 132],
      [1030, 226],
      [936, 268],
    ],
    [
      [420, 640],
      [500, 596],
      [566, 650],
      [520, 720],
    ],
  ];
  return { stars, groups };
})();

/* ---------- 小工具 ---------- */
export const fmtTime = (iso?: string | null) => {
  if (!iso) return "—";
  const d = new Date(iso);
  if (Number.isNaN(d.getTime())) return iso;
  const p = (n: number) => String(n).padStart(2, "0");
  const dt = `${p(d.getMonth() + 1)}-${p(d.getDate())}`;
  return `${dt} ${p(d.getHours())}:${p(d.getMinutes())}`;
};
export const fmtDay = fmtTime;
export const pct = (v: number) => `${(v * 100).toFixed(2)}%`;
export const rarVar = (r: string) =>
  `var(--${r === "N" ? "nrm" : r.toLowerCase()})`;
export const daysLeft = (endsAt?: string | null) => {
  if (!endsAt) return null;
  const ms = new Date(endsAt).getTime() - Date.now();
  if (!Number.isFinite(ms) || ms <= 0) return 0;
  return Math.ceil(ms / 86400000);
};
export const ago = (iso?: string | null) => {
  if (!iso) return "—";
  const ms = Date.now() - new Date(iso).getTime();
  if (!Number.isFinite(ms)) return "—";
  const m = Math.floor(ms / 60000);
  if (m < 1) return "刚刚";
  if (m < 60) return `${m} 分钟前`;
  const h = Math.floor(m / 60);
  if (h < 24) return `${h} 小时前`;
  return fmtTime(iso);
};

/* ---------- 弹窗骨架 ---------- */

export function RuleStep({
  no,
  b,
  span,
}: {
  no: string;
  b: string;
  span: React.ReactNode;
}) {
  return (
    <div className="rule-step">
      <span className="no">{no}</span>
      <div>
        <b>{b}</b>
        <span>{span}</span>
      </div>
    </div>
  );
}

export function DotMatrix({ lit, total }: { lit: number; total: number }) {
  const cells = Math.min(total, 48);
  return (
    <div className="dotmatrix" aria-hidden="true">
      {Array.from({ length: cells }, (_, i) => (
        <i
          key={i}
          className={i < lit - 1 ? "lit" : i === lit - 1 ? "gold" : ""}
        />
      ))}
    </div>
  );
}

/* ---------- 星盘保底环 ---------- */
export function Astro({
  pity,
  soft,
  hard,
}: {
  pity: number;
  soft: number;
  hard: number;
}) {
  const c = 180;
  const hardLeft = Math.max(0, hard - pity);
  const done = Math.max(0, Math.min(hard, pity));
  const softDone = Math.max(0, Math.min(soft, pity));
  const circ = 2 * Math.PI * 141;
  const arcLen = ((circ * done) / hard).toFixed(1);
  const dash = `${arcLen} ${circ.toFixed(1)}`;
  return (
    <svg className="astro" viewBox="0 0 360 360" aria-hidden="true">
      {[170, 112].map((r, i) => (
        <circle
          key={r}
          cx={c}
          cy={c}
          r={r}
          fill="none"
          stroke="var(--t2)"
          strokeWidth="1"
          opacity={i === 0 ? 0.18 : 0.14}
        />
      ))}
      <circle
        cx={c}
        cy={c}
        r={148}
        fill="none"
        stroke="var(--t2)"
        strokeWidth="1"
        opacity="0.1"
        strokeDasharray="1 7"
      />
      <circle
        cx={c}
        cy={c}
        r={141}
        fill="none"
        stroke="var(--gold)"
        strokeWidth="3.4"
        strokeLinecap="round"
        opacity="0.55"
        strokeDasharray={dash}
        transform={`rotate(-90 ${c} ${c})`}
      />
      {Array.from({ length: 90 }, (_, i) => {
        const a = (i / 90) * Math.PI * 2 - Math.PI / 2;
        const r1 = 158;
        const r2 = i < done ? 170 : 164;
        return (
          <line
            key={`h${i}`}
            x1={+(c + Math.cos(a) * r1).toFixed(1)}
            y1={+(c + Math.sin(a) * r1).toFixed(1)}
            x2={+(c + Math.cos(a) * r2).toFixed(1)}
            y2={+(c + Math.sin(a) * r2).toFixed(1)}
            stroke={i < done ? "var(--gold)" : "var(--t3)"}
            strokeWidth={i % 10 === 0 ? 2 : 1}
            opacity={i < done ? 0.92 : 0.28}
          />
        );
      })}
      {Array.from({ length: 60 }, (_, j) => {
        const b = (j / 60) * Math.PI * 2 - Math.PI / 2;
        const on = j < softDone;
        return (
          <line
            key={`s${j}`}
            x1={+(c + Math.cos(b) * 120).toFixed(1)}
            y1={+(c + Math.sin(b) * 120).toFixed(1)}
            x2={+(c + Math.cos(b) * (on ? 130 : 126)).toFixed(1)}
            y2={+(c + Math.sin(b) * (on ? 130 : 126)).toFixed(1)}
            stroke={on ? "var(--star)" : "var(--t3)"}
            strokeWidth={j % 5 === 0 ? 1.8 : 0.9}
            opacity={on ? 0.8 : 0.2}
          />
        );
      })}
      <AstroDot done={done} hard={hard} r={170} size={3.2} fill />
      <AstroDot done={done} hard={hard} r={170} size={7} ring />
      {hardLeft === 0 && (
        <circle
          cx={c}
          cy={c}
          r={176}
          fill="none"
          stroke="var(--gold-l)"
          strokeWidth="1"
          opacity="0.35"
        />
      )}
    </svg>
  );
}

export function AstroDot({
  done,
  hard,
  r,
  size,
  fill,
  ring,
}: {
  done: number;
  hard: number;
  r: number;
  size: number;
  fill?: boolean;
  ring?: boolean;
}) {
  const ang = (done / hard) * Math.PI * 2 - Math.PI / 2;
  const cx = +(180 + Math.cos(ang) * r).toFixed(1);
  const cy = +(180 + Math.sin(ang) * r).toFixed(1);
  return (
    <circle
      cx={cx}
      cy={cy}
      r={size}
      fill={fill ? "var(--gold-l)" : "none"}
      stroke={ring ? "var(--gold-l)" : "none"}
      strokeWidth={ring ? 1 : 0}
      opacity={ring ? 0.45 : 1}
    />
  );
}
