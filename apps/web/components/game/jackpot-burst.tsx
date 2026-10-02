"use client";

import { useEffect, useState } from "react";

/**
 * 大奖庆祝时刻（趣味性批）：kind=jackpot 时全屏彩带 + 金色大字。
 * 纯 CSS 动画（prefers-reduced-motion 下降级为无彩带）；2.4s 自动收场。
 * 挂在 ResultFlash 的上层 —— 各玩法页零改动，只在 ResultFlash 里渲染。
 */

const CONFETTI_N = 26;
// 甜梦调色板：香槟金 / 冰蓝 / 淡紫 / 薄荷（深色页面也看得见）
const COLORS = ["#d4af7a", "#f0d9a8", "#8ab4dd", "#b8c8e8", "#8ec6a2"];

interface Piece {
  left: number;
  delay: number;
  dur: number;
  color: string;
  size: number;
  rot: number;
}

function roll(): Piece[] {
  // SSR/首帧定死一批（hydration 稳定）：伪随机用确定性种子序列
  const out: Piece[] = [];
  let seed = 7;
  const rnd = () => {
    seed = (seed * 9301 + 49297) % 233280;
    return seed / 233280;
  };
  for (let i = 0; i < CONFETTI_N; i++) {
    out.push({
      left: Math.round(rnd() * 100),
      delay: Math.round(rnd() * 500),
      dur: 1400 + Math.round(rnd() * 900),
      color: COLORS[i % COLORS.length],
      size: 6 + Math.round(rnd() * 6),
      rot: Math.round(rnd() * 360),
    });
  }
  return out;
}

export function JackpotBurst({
  fire,
  label,
}: {
  /** 变 true 的瞬间开演；组件内部自清 */
  fire: boolean;
  /** 无障碍标签（与飘字同文案） */
  label: string;
}) {
  const [pieces] = useState<Piece[]>(roll);
  const [on, setOn] = useState(false);
  useEffect(() => {
    if (!fire) return;
    setOn(true);
    const id = window.setTimeout(() => setOn(false), 2400);
    return () => window.clearTimeout(id);
  }, [fire]);
  if (!on) return null;
  return (
    <div className="jpk-burst" role="presentation" aria-label={label}>
      {pieces.map((p, i) => (
        <span
          key={i}
          className="jpk-piece"
          style={{
            left: `${p.left}%`,
            background: p.color,
            width: p.size,
            height: p.size * 0.6,
            animationDelay: `${p.delay}ms`,
            animationDuration: `${p.dur}ms`,
            transform: `rotate(${p.rot}deg)`,
          }}
        />
      ))}
    </div>
  );
}
