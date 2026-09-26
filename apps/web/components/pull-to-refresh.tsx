"use client";

import { useEffect, useRef, useState } from "react";

/** 下拉刷新（M3，方案 §交互规范）：仅触屏态启用（桌面不渲染）。
 *  下拉 64px 触发 onRefresh，指示器用 TIDE 波浪符；
 *  页面在滚动顶部（scrollTop ≤ 2）才接管触摸，不劫持正常滚动。
 *  用法：<PullToRefresh onRefresh={() => router.refresh()} /> 挂列表页顶层。 */
export function PullToRefresh({
  onRefresh,
  label,
}: {
  onRefresh: () => void | Promise<void>;
  label: string;
}) {
  const [dist, setDist] = useState(0);
  const [busy, setBusy] = useState(false);
  const [touch, setTouch] = useState(false);
  const startY = useRef<number | null>(null);

  // 仅触屏：首次 touchstart 后才启用（SSR 首帧不渲染指示器）
  useEffect(() => {
    const onFirstTouch = () => setTouch(true);
    window.addEventListener("touchstart", onFirstTouch, { once: true });
    return () =>
      window.removeEventListener("touchstart", onFirstTouch);
  }, []);

  if (!touch) return null;

  const atTop = () =>
    (document.scrollingElement?.scrollTop ?? window.scrollY) <= 2;

  const ready = dist >= 64;
  return (
    <div
      className="ptr"
      style={{ height: busy ? 44 : Math.min(dist, 96) }}
      aria-live="polite"
      aria-label={label}
      onTouchStart={(e) => {
        if (!atTop() || busy) return;
        startY.current = e.touches[0].clientY;
      }}
      onTouchMove={(e) => {
        if (startY.current === null || busy) return;
        const dy = e.touches[0].clientY - startY.current;
        if (dy > 0) setDist(dy);
      }}
      onTouchEnd={async () => {
        if (startY.current === null) return;
        startY.current = null;
        if (ready) {
          setBusy(true);
          setDist(64);
          try {
            await onRefresh();
          } finally {
            setBusy(false);
            setDist(0);
          }
        } else {
          setDist(0);
        }
      }}
    >
      {(dist > 0 || busy) && (
        <span
          className={`ptr__ind ${ready || busy ? "is-ready" : ""} ${
            busy ? "is-busy" : ""
          }`}
        >
          {busy ? "≈" : ready ? "≈" : "~"}
        </span>
      )}
    </div>
  );
}
