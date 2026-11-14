"use client";

import { useEffect, useRef, useState } from "react";
import { playSfx } from "@/lib/sfx";

/**
 * 星尘信封 · 3×2 涂层格（样图②）：一局 = 一张信封，六个涂层格，
 * 其中一格藏着本张结果（服务端在买卡时已定死），其余是谢谢参与。
 * 纯呈现层 —— 刮开的顺序由玩家选，结果不因顺序改变。
 *
 * 手感口径（.zcode/skills/arcade-motion-vfx）：单格手刮即时翻开 + pop 一声；
 * 一次新增 ≥2 格（一键全开）→ data-stagger，CSS 逐格 40ms 错峰，悬念感来自延迟。
 */

export interface CoatOutcome {
  mult: number;
  icon?: string;
  rarity: number;
  /** 本张净收（flash 文案用，格子不显示） */
  net: number;
}

export function ScratchCoat({
  outcome,
  hitIndex,
  revealed,
  onReveal,
  decoyLabel,
  disabled,
}: {
  outcome: CoatOutcome | null;
  /** 奖品格下标（buy 时随机定，与结果无关纯粹是揭开仪式） */
  hitIndex: number | null;
  revealed: number[];
  onReveal: (i: number) => void;
  decoyLabel: string;
  disabled?: boolean;
}) {
  const cells = [0, 1, 2, 3, 4, 5];
  // 上一帧已开格数：一次 render 内增幅 ≥2 即批量揭开（一键全开）
  const prevOpen = useRef(0);
  const [stagger, setStagger] = useState(false);
  useEffect(() => {
    const delta = revealed.length - prevOpen.current;
    prevOpen.current = revealed.length;
    if (delta >= 2) {
      setStagger(true);
      const id = window.setTimeout(() => setStagger(false), 800);
      return () => window.clearTimeout(id);
    }
    if (delta === 1) playSfx("pop");
  }, [revealed.length]);

  return (
    <div className="sw-coat-grid" data-stagger={stagger || undefined}>
      {cells.map((i) => {
        const open = revealed.includes(i);
        const isHit = open && i === hitIndex && outcome !== null;
        return (
          <button
            key={i}
            type="button"
            className={`sw-coat-cell${open ? " open" : ""}${
              isHit ? " sw-coat-hit" : ""
            }`}
            disabled={disabled || open || outcome === null}
            onClick={() => onReveal(i)}
            aria-label={open ? undefined : "scratch"}
          >
            <span className="sw-coat-face">
              {isHit ? (
                <>
                  <span className="ic" aria-hidden>
                    {outcome?.icon || "✨"}
                  </span>
                  <span className="mx num">
                    {outcome?.mult.toFixed(1)}x
                  </span>
                </>
              ) : open ? (
                <span className="decoy">{decoyLabel}</span>
              ) : (
                <span aria-hidden>?</span>
              )}
            </span>
            {!open && <span className="sw-coat-lid" aria-hidden>刮</span>}
          </button>
        );
      })}
    </div>
  );
}
