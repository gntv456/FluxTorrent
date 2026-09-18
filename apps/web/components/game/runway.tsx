"use client";

import { useEffect, useMemo, useRef, useState } from "react";

/**
 * 猜大小开奖跑道：1-100 三段着色 + 游标减速定格 + 数字 odometer。
 * 全程只有一个 CSS transition（不写 JS 逐帧），reduced-motion 时直接跳到结果。
 */
export function RunwayOdometer({
  number,
  spinning,
  reduced,
}: {
  number: number | null;
  spinning: boolean;
  reduced: boolean;
}) {
  const [shown, setShown] = useState<number | null>(null);
  const timer = useRef<number | null>(null);

  // 结果回来后才启动动画（修「动画与请求时长解耦」）：先跑到随机位，再定格到真实值
  useEffect(() => {
    if (number === null) return;
    if (reduced) {
      setShown(number);
      return;
    }
    setShown(1 + Math.floor(Math.random() * 100));
    timer.current = window.setTimeout(() => setShown(number), 60);
    return () => {
      if (timer.current) window.clearTimeout(timer.current);
    };
  }, [number, reduced]);

  const digits = useMemo(() => {
    const s = String(shown ?? 0).padStart(3, "0");
    return [s[0], s[1], s[2]];
  }, [shown]);

  const pct = ((Math.max(1, shown ?? 1) - 1) / 99) * 100;

  return (
    <div className="w-full max-w-[420px]">
      <div className="relative h-3.5 overflow-hidden rounded-full border border-line bg-[linear-gradient(90deg,var(--sky)_0%,var(--sky)_47%,var(--surface-sunken)_47%,var(--surface-sunken)_51%,var(--coral)_51%,var(--coral)_100%)]">
        <span
          aria-hidden
          className="absolute -top-2 h-0 w-0 border-x-[7px] border-t-[10px] border-x-transparent border-t-ink transition-[left] duration-[1600ms] ease-[cubic-bezier(.16,.84,.44,1)]"
          style={{ left: `${pct}%`, transform: "translateX(-7px)" }}
        />
      </div>
      <div className="mt-1.5 flex justify-between text-[11px] text-sub">
        <span>1</span>
        <span>49</span>
        <span>50-51</span>
        <span>52</span>
        <span>100</span>
      </div>
      <div className="mt-4 flex justify-center gap-1.5" aria-hidden>
        {digits.map((d, i) => (
          <span
            key={i}
            className="relative h-[52px] w-[36px] overflow-hidden rounded-[var(--r-sm)] border border-line bg-[var(--surface-card)]"
          >
            <span
              className="absolute left-0 top-0 flex w-full flex-col transition-transform duration-[1600ms] ease-[cubic-bezier(.16,.84,.44,1)]"
              style={{ transform: `translateY(${-52 * Number(d)}px)` }}
            >
              {[0, 1, 2, 3, 4, 5, 6, 7, 8, 9].map((n) => (
                <span
                  key={n}
                  className="num flex h-[52px] items-center justify-center text-[28px] font-black"
                >
                  {n}
                </span>
              ))}
            </span>
          </span>
        ))}
      </div>
      <p className="sr-only" role="status" aria-live="polite">
        {spinning ? "开奖中" : number === null ? "等待下注" : `开奖 ${number}`}
      </p>
    </div>
  );
}
