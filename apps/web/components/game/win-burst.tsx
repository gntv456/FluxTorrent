"use client";

import { useEffect, useState } from "react";

/**
 * 普通赢局庆祝（中档）：星屑小迸发，从飘字位置向上散开。
 * 庆祝分级口径（.zcode/skills/arcade-motion-vfx）：
 *   轻 lose/tie（无特效）→ 中 win（本件 ~10 粒 1.2s）→ 全 jackpot（JackpotBurst 26 片 2.4s）
 * 确定性伪随机（hydration 稳定）+ 播完即卸；reduced-motion 由 CSS 降级。
 * 挂在 ResultFlash 的 kind=win 上——各玩法页零改动。
 */

const SPARK_N = 10;
// 甜梦调色板与 JackpotBurst 同源：香槟金 / 冰蓝 / 薄荷
const COLORS = ["#d4af7a", "#f0d9a8", "#8ab4dd", "#8ec6a2"];

interface Spark {
  dx: number; // 水平散开 px
  delay: number; // ms
  dur: number; // ms
  color: string;
  size: number;
}

function roll(): Spark[] {
  const out: Spark[] = [];
  let seed = 11;
  const rnd = () => {
    seed = (seed * 9301 + 49297) % 233280;
    return seed / 233280;
  };
  for (let i = 0; i < SPARK_N; i++) {
    out.push({
      dx: Math.round((rnd() - 0.5) * 160),
      delay: Math.round(rnd() * 180),
      dur: 900 + Math.round(rnd() * 400),
      color: COLORS[i % COLORS.length],
      size: 4 + Math.round(rnd() * 3),
    });
  }
  return out;
}

export function WinBurst({ fire }: { fire: boolean }) {
  const [sparks] = useState<Spark[]>(roll);
  const [on, setOn] = useState(false);
  useEffect(() => {
    if (!fire) return;
    setOn(true);
    const id = window.setTimeout(() => setOn(false), 1400);
    return () => window.clearTimeout(id);
  }, [fire]);
  if (!on) return null;
  return (
    <span className="wburst" role="presentation" aria-hidden>
      {sparks.map((p, i) => (
        <span
          key={i}
          className="wburst-spark"
          style={{
            // CSS 变量携带散开距离，keyframes 里只用 transform 消费
            ["--wdx" as string]: `${p.dx}px`,
            background: p.color,
            width: p.size,
            height: p.size,
            animationDelay: `${p.delay}ms`,
            animationDuration: `${p.dur}ms`,
          }}
        />
      ))}
    </span>
  );
}
