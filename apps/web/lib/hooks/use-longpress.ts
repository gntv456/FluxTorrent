"use client";

import { useRef, useState } from "react";

/** 长按多选 hook（M4，方案 §交互规范）：管理端列表长按（500ms）进入
 *  多选态；再次长按或点「退出」退出。仅触屏（鼠标用户用 checkbox）。
 *  用法：const [multi, bindLong] = useLongPressMultiSelect();
 *  bindLong 挂到行容器（onPointerDown/Up/Cancel）。 */
export function useLongPressMultiSelect(): [
  boolean,
  {
    onPointerDown: (e: React.PointerEvent) => void;
    onPointerUp: () => void;
    onPointerCancel: () => void;
  },
  () => void,
] {
  const [multi, setMulti] = useState(false);
  const timer = useRef<number | null>(null);

  const clear = () => {
    if (timer.current !== null) {
      window.clearTimeout(timer.current);
      timer.current = null;
    }
  };
  const bind = {
    onPointerDown: (e: React.PointerEvent) => {
      if (e.pointerType !== "touch" || multi) return;
      clear();
      timer.current = window.setTimeout(() => {
        // 触觉反馈（支持的浏览器）
        if (navigator.vibrate) navigator.vibrate(10);
        setMulti(true);
      }, 500);
    },
    onPointerUp: clear,
    onPointerCancel: clear,
  };
  return [multi, bind, () => setMulti(false)];
}
