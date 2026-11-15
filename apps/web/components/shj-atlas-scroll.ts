"use client";

import { useRef } from "react";

/** 图鉴侧栏滚动条静止隐藏（2026-11-15）：滚动中给容器挂 .is-scrolling
 *  恢复 thin 滚动条，停 900ms 摘类——常驻滚动条在玉青底上是一条亮线。
 *  计时器按容器分存（WeakMap：scroll 区与多个浮层互不干扰），
 *  组件卸载后 WeakMap 随宿主回收，无需手动清理。 */
export function useScrollingClass() {
  const timers = useRef<WeakMap<HTMLElement, number>>(new WeakMap());
  return (e: React.UIEvent<HTMLElement>) => {
    const el = e.currentTarget;
    el.classList.add("is-scrolling");
    const map = timers.current;
    window.clearTimeout(map.get(el));
    map.set(
      el,
      window.setTimeout(() => el.classList.remove("is-scrolling"), 900),
    );
  };
}
