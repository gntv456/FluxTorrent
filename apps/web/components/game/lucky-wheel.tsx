"use client";

import { useEffect, useRef, useState } from "react";
import { WheelDisc } from "@/components/game/wheel-disc";
import type { WheelPrize } from "@/lib/wheel-prize";

const SPIN_MS = 4000;

/**
 * 大转盘：指针固定在顶部，盘面顺时针旋转后停格。
 * 结果在响应返回后才开始旋转（与九宫格同纪律）。
 * 盘面绘制拆到 wheel-disc（纯 SVG），这里只管旋转/停格/中心钮。
 */
export function LuckyWheel({
  prizes,
  ticket,
  resultIndex,
  spinKey,
  busy,
  disabled,
  reduced,
  onDraw,
  onLanded,
  goLabel,
  spinningLabel,
  tapLabel,
}: {
  prizes: WheelPrize[];
  ticket: number;
  resultIndex: number | null;
  /** 发数序号：结果扇区与上一把相同时也必须重转 */
  spinKey: number;
  busy: boolean;
  disabled?: boolean;
  reduced: boolean;
  onDraw: () => void;
  onLanded?: () => void;
  goLabel: string;
  spinningLabel: string;
  /** 中心毂副行（样图「点击开始」） */
  tapLabel: string;
}) {
  const [rot, setRot] = useState(0);
  const [hit, setHit] = useState<number | null>(null);
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
    setHit(null);
    if (reduced) {
      // 减少动效：直接落到最终扇区，不转圈，仍呈现结果位置
      const center = resultIndex * seg + seg / 2;
      setRot((360 - center) % 360);
      setHit(resultIndex);
      onLanded?.();
      return;
    }
    // 让该扇区中心停在顶部指针（旋转角 ≡ -(扇区中心) mod 360）
    const center = resultIndex * seg + seg / 2;
    const desired = (360 - center) % 360;
    setRot((prev) => {
      const cur = ((prev % 360) + 360) % 360;
      const delta = (((desired - cur) % 360) + 360) % 360;
      return prev + 360 * 5 + delta;
    });
    timer.current = window.setTimeout(() => {
      setHit(resultIndex);
      onLanded?.();
    }, SPIN_MS);
    return () => {
      if (timer.current) window.clearTimeout(timer.current);
    };
    // eslint-disable-next-line react-hooks/exhaustive-deps
  }, [spinKey, reduced]);

  return (
    <div className="lw-rig wp-lw-rig">
      <div className="wp-halo" aria-hidden />
      <div className="wp-rays" aria-hidden />
      <div className="wp-reflect" aria-hidden />
      <div className="lw-disc-wrap">
        <span className="lw-pointer" aria-hidden />
        <div
          className={`lw-disc-rotor${reduced ? "" : " spin"}`}
          style={{ transform: `rotate(${rot}deg)` }}
        >
          <WheelDisc prizes={prizes} ticket={ticket} hitIndex={hit} />
        </div>
        <button
          type="button"
          onClick={onDraw}
          disabled={busy || disabled}
          className="lw-hub"
          style={{
            background:
              "radial-gradient(circle at 35% 28%, " +
              "var(--mood-wheel-face-a), var(--mood-wheel-face-b) 62%, " +
              "var(--mood-wheel-face-c))",
            border: "3px solid var(--mood-wheel-hub-b)",
            boxShadow:
              "0 6px 14px var(--mood-wheel-hub-shadow), " +
              "inset 0 1px 2px rgba(255,255,255,0.85)",
            color: "var(--mood-wheel-text-ink)",
          }}
          aria-label={`${goLabel} · ${ticket}`}
        >
          <span className="lw-go">{busy ? spinningLabel : goLabel}</span>
          {!busy && <small>{tapLabel}</small>}
        </button>
      </div>
    </div>
  );
}
