"use client";

import { useEffect, useRef } from "react";

/**
 * 魔力余额变更广播（同仓 `REPLY_EVENT` / `THANK_EVENT` 的事件惯例）。
 *
 * 顶栏魔力 chip 的值是 `user-menu` 进页面时读 `/me`+`/me/spark` 拿到的本地
 * state；任何让余额变化的写操作若在成功后不广播，顶栏就停在旧值——用户刚
 * 签完到（后端已 +8 入账）看到的仍是签到前的数字，等于「成功了却像没成功」
 * （2026-10-21 实测：接口 8339→8347、attendance 已落今天，顶栏仍显 8,339，
 * 手动刷新才对上）。
 */
export const SPARK_EVENT = "flux:spark-changed";

/** 写操作成功后调用一次；SSR 阶段无 window，直接跳过。 */
export function notifySparkChanged(): void {
  if (typeof window === "undefined") return;
  window.dispatchEvent(new Event(SPARK_EVENT));
}

/** 订阅方：余额一变就跑 `on`（通常是重读 `/me/spark`）。
 *  回调经 ref 转手，避免因父组件每次渲染都换函数引用而反复重绑监听。 */
export function useSparkSync(on: () => void): void {
  const ref = useRef(on);
  ref.current = on;
  useEffect(() => {
    const fire = () => ref.current();
    window.addEventListener(SPARK_EVENT, fire);
    return () => window.removeEventListener(SPARK_EVENT, fire);
  }, []);
}
