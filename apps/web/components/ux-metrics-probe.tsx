"use client";

import { useEffect } from "react";
import { uxMark, isUxMetricName } from "@/lib/ux-metrics";

/** 全局体验探针：app_ready（水合完成）+ LCP（最大内容绘制）+ 事件委托。
 *  事件委托让服务端组件也能埋点：任意元素加 data-ux-event 即可，
 *  无需把它改成 client component；只写本地，不发包。 */
export function UxMetricsProbe() {
  useEffect(() => {
    uxMark("app_ready");
    if (typeof PerformanceObserver !== "undefined") {
      try {
        const po = new PerformanceObserver((list) => {
          for (const e of list.getEntries()) {
            uxMark("lcp", undefined, Math.round(e.startTime));
          }
        });
        po.observe({ type: "largest-contentful-paint", buffered: true });
        return () => po.disconnect();
      } catch {
        /* 不支持 LCP 观察则跳过，不影响其余埋点 */
      }
    }
    const onClick = (e: MouseEvent) => {
      const el = (e.target as HTMLElement | null)?.closest(
        "[data-ux-event]",
      );
      if (!el) return;
      const name = el.getAttribute("data-ux-event") ?? "";
      if (!isUxMetricName(name)) return;
      uxMark(name, el.getAttribute("data-ux-label") ?? undefined);
    };
    document.addEventListener("click", onClick);
    return () => document.removeEventListener("click", onClick);
  }, []);
  return null;
}
