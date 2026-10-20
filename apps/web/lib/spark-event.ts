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

/**
 * 会改动本人魔力余额的后端写口（路径不含 `/api/v1` 前缀；`{id}` 为通配段）。
 *
 * 这张表**不是**给组件逐个手挂的清单——所有请求都过 `lib/api-client.ts`
 * 的 `request()`，在那一处按表广播即可全口生效；组件里再各写一遍反而会漏
 * （实测 68 个写口里只有签到与十来处 `router.refresh()` 有动作，而顶栏余额
 * 是 `user-menu` 里 `useEffect(…, [])` 读的本地 state，`router.refresh()`
 * 根本不会重跑它 ⇒ 刷新了数据也刷新不了数字）。
 * 表内容与后端余额原语的对账由 `scripts/pt_audit_spark_sync.py` 常驻守住：
 * 新增一个会扣/加魔力的写口而忘记登记，闸门直接红。
 */
export const SPARK_WRITE_PATHS: string[] = [
  "/attendance/checkin", "/bank/demand/deposit", "/bank/demand/withdraw",
  "/bank/deposit", "/bank/loan/apply", "/bank/loan/repay", "/bank/withdraw",
  "/downloads/keys", "/downloads/keys/revoke", "/farm/craft",
  "/farm/craft/collect", "/farm/fertilize", "/farm/harvest", "/farm/land/buy",
  "/farm/land/upgrade", "/farm/plant", "/farm/ranch/buy",
  "/farm/ranch/collect", "/farm/ranch/feed", "/farm/water",
  "/forums/bounty/award", "/forums/lottery/draw", "/forums/lottery/join",
  "/forums/posts/{id}/like", "/forums/tip", "/forums/topics",
  "/forums/topics/{id}/favorite", "/forums/topics/{id}/reply", "/fun/vote",
  "/fundings/contribute", "/games/arcade/backpack/use", "/games/bigsmall",
  "/games/capsule", "/games/fishing/cast", "/games/fishing/reel",
  "/games/fishing/rod/upgrade", "/games/jgg", "/games/pet/claim",
  "/games/pet/feed", "/games/scratch", "/games/wheel", "/gomoku/games",
  "/gomoku/games/{id}/join", "/gomoku/games/{id}/move", "/invites",
  "/invites/redeem", "/jixiao/claim", "/magic-pool/donate", "/me/hr/pardon",
  "/medals/buy", "/medals/gift", "/offers", "/offers/promote", "/offers/vote",
  "/promo/buy", "/requests", "/requests/fulfill", "/shop/buy", "/subtitles",
  "/subtitles/requests", "/subtitles/requests/{id}/accept",
  "/subtitles/requests/{id}/cancel", "/subtitles/requests/{id}/contribute",
  "/subtitles/requests/{id}/crew-accept", "/subtitles/requests/{id}/deliver",
  "/tasks/claim", "/torrents/batch-download", "/torrents/{id}/download-link",
];

/** 逐段比较，`{xxx}` 段视作通配（避开正则转义坑）。 */
function pathMatches(entry: string, actual: string): boolean {
  const a = entry.split("/");
  const b = actual.split("/");
  if (a.length !== b.length) return false;
  return a.every((seg, i) => seg.startsWith("{") || seg === b[i]);
}

/** http 层出口：写方法 + 命中登记表 ⇒ 广播一次，顶栏余额跟着回读。
 *  宁可对「只给作者加钱、本人余额不变」的口也广播一次（成本是一趟
 *  `/me/spark`），也不让真扣了魔力的口静默留着旧数字。 */
export function maybeNotifySparkChanged(
  path: string,
  method?: string
): void {
  if (!/^(post|put|patch|delete)$/i.test(method || "")) return;
  const bare = path.split("?")[0].replace(/^\/api\/v1/, "");
  if (!SPARK_WRITE_PATHS.some((e) => pathMatches(e, bare))) return;
  notifySparkChanged();
}
