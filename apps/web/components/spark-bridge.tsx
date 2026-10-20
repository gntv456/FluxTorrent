"use client";

import { useRouter } from "next/navigation";
import { useSparkSync } from "@/lib/spark-event";

/**
 * 余额广播的统一消费者——挂在根 layout 一次，全站生效。
 *
 * 为什么不能只靠顶栏 chip 自己重读：
 *  - 顶栏数字可被站点开关 `topbar_spark_chip` 关掉（现网实测 = `no`，
 *    站长 2026-10-20 定的口径），此时用户看见的余额是**首页与个人页里
 *    SSR 直出的那几个数**（`/home` 的 site_data 与 `/me/overview`）；
 *  - 那串数是服务端渲染进来的，客户端不会自己回读，只有 `router.refresh()`
 *    会把当前路由的 RSC 重新取一遍。
 * 反过来，`user-menu` 里的 chip 是 `useEffect(…, [])` 存的本地 state，
 * `router.refresh()` 也不会重跑它。
 * 所以两处各自订阅、分工不同：chip 走重读 `/me/spark`，SSR 数字走 refresh。
 */
export function SparkBridge() {
  const router = useRouter();
  useSparkSync(() => router.refresh());
  return null;
}
