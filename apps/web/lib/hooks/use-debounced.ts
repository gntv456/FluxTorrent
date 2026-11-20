"use client";

import { useEffect, useState } from "react";

/** 输入防抖（2026-11-20 五问评审 P1）：三处搜索（短讯/求种/字幕）
 *  此前每次击键都发一次列表请求——弱网下输入框发飘、结果跳变、服务端被打。
 *  用法：const committed = useDebounced(draft, 400)，拿 committed 进加载依赖。 */
export function useDebounced<T>(value: T, delay = 400): T {
  const [settled, setSettled] = useState(value);
  useEffect(() => {
    const id = window.setTimeout(() => setSettled(value), delay);
    return () => window.clearTimeout(id);
  }, [value, delay]);
  return settled;
}
