"use client";

import { useEffect, useMemo, useRef, useState } from "react";
import { useI18n } from "@/i18n/client";

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
  const { dict } = useI18n();
  const bs = dict.games.bigsmall;
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
    <div className="bs-run">
      <div className="bs-track">
        <span
          aria-hidden
          className="bs-cursor"
          style={{ left: `${pct}%` }}
        />
      </div>
      <div className="bs-ticks">
        <span>1</span>
        <span>49</span>
        <span>50-51</span>
        <span>52</span>
        <span>100</span>
      </div>
      <div className="bs-plates" aria-hidden>
        {digits.map((d, i) => (
          <span key={i} className="bs-plate">
            <span
              className="roll"
              style={{ transform: `translateY(${-52 * Number(d)}px)` }}
            >
              {[0, 1, 2, 3, 4, 5, 6, 7, 8, 9].map((n) => (
                <span key={n} className="num d">
                  {n}
                </span>
              ))}
            </span>
          </span>
        ))}
      </div>
      <p className="sr-only" role="status" aria-live="polite">
        {spinning
          ? bs.pending
          : number === null
            ? bs.srWait
            : bs.srResult.replace("{n}", String(number))}
      </p>
    </div>
  );
}
