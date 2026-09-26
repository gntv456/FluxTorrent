"use client";

import { useCallback, useRef, useState } from "react";

/** 左滑快捷操作 hook（M3，方案 §交互规范）：
 *  行左滑露出 ≤2 个操作，行程 64px/个（max 128px），松手过半自动吸合。
 *  仅触屏（pointerType==="touch"）——鼠标拖拽不误触。
 *  用法：const [offset, bind] = useSwipe(open, setOpen)；bind 展开
 *  到行前置层，style={{ transform: translateX(offset) }}。 */
export function useSwipe(
  open: boolean,
  setOpen: (v: boolean) => void,
): [number, Record<string, unknown>] {
  const [offset, setOffset] = useState(0);
  const startX = useRef<number | null>(null);

  const bind = {
    onPointerDown: useCallback((e: React.PointerEvent) => {
      if (e.pointerType !== "touch") return;
      startX.current = e.clientX;
    }, []),
    onPointerMove: useCallback(
      (e: React.PointerEvent) => {
        if (startX.current === null) return;
        const dx = e.clientX - startX.current;
        if (dx < 0) setOffset(Math.max(dx, -128));
      },
      [],
    ),
    onPointerUp: useCallback(() => {
      if (startX.current === null) return;
      startX.current = null;
      // 过半吸合（-64px 阈值）
      if (offset < -64) {
        setOffset(-128);
        setOpen(true);
      } else {
        setOffset(0);
        setOpen(false);
      }
    }, [offset, setOpen]),
    onPointerCancel: useCallback(() => {
      startX.current = null;
      setOffset(open ? -128 : 0);
    }, [open]),
  };
  return [offset, bind];
}
