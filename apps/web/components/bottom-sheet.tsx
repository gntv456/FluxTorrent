"use client";

import { useEffect, useRef, useState, type ReactNode } from "react";

import { useFocusTrap } from "@/lib/use-focus-trap";

/** Bottom Sheet 通用件（M3，移动端方案 §交互规范）：
 *  圆角 18 · grabber · max-height 82% · 遮罩 rgba(8,19,31,.42)。
 *  关闭：下拉 grabber 区 ≥64px / 点遮罩 / ESC。打开锁 body 滚动。
 *  仅 <md 渲染由调用方控制（外层 md:hidden），本组件不判断点。 */
export function BottomSheet({
  open,
  onClose,
  title,
  children,
}: {
  open: boolean;
  onClose: () => void;
  title: string;
  children: ReactNode;
}) {
  const [dragY, setDragY] = useState(0);
  const startY = useRef<number | null>(null);
  const sheetRef = useRef<HTMLDivElement>(null);
  // 焦点圈定：Tab 不穿透到遮罩后的页面（ZT81）
  useFocusTrap(open, sheetRef);

  useEffect(() => {
    if (!open) return;
    const prev = document.body.style.overflow;
    document.body.style.overflow = "hidden";
    const onKey = (e: KeyboardEvent) => {
      if (e.key === "Escape") onClose();
    };
    document.addEventListener("keydown", onKey);
    return () => {
      document.body.style.overflow = prev;
      document.removeEventListener("keydown", onKey);
    };
  }, [open, onClose]);

  if (!open) return null;

  const onDown = (e: React.PointerEvent) => {
    startY.current = e.clientY;
  };
  const onMove = (e: React.PointerEvent) => {
    if (startY.current === null) return;
    setDragY(Math.max(0, e.clientY - startY.current));
  };
  const onUp = () => {
    if (dragY > 64) onClose();
    setDragY(0);
    startY.current = null;
  };

  return (
    <div
      className="bsheet-root"
      role="dialog"
      aria-modal="true"
      aria-label={title}
    >
      <div className="bsheet-mask" onClick={onClose} />
      <div
        ref={sheetRef}
        className="bsheet"
        style={dragY ? { transform: `translateY(${dragY}px)` } : undefined}
      >
        <div
          className="bsheet__grab"
          onPointerDown={onDown}
          onPointerMove={onMove}
          onPointerUp={onUp}
          onPointerCancel={onUp}
        >
          <span className="bsheet__bar" aria-hidden />
          <p className="bsheet__title">{title}</p>
        </div>
        <div className="bsheet__body">{children}</div>
      </div>
    </div>
  );
}
