"use client";

import { useEffect, useState } from "react";
import { useI18n } from "@/i18n/client";

/** 维护页倒计时（0462）：`until` 是后端校验过 RFC3339 的字符串。
 *  到点后不再显示负数——改显示「恢复中」，站长关闸那一刻页面自然回来。
 *  这里**不**自动刷新页面：维护结束的信号由站长关闸给出，倒计时只是
 *  「预计」，死循环 reload 打不打得开都难看；用户手动刷新即可。 */
export function MaintenanceCountdown({ until }: { until: string }) {
  const { dict } = useI18n();
  const [left, setLeft] = useState(() => untilSeconds(until));
  useEffect(() => {
    const t = setInterval(() => setLeft(untilSeconds(until)), 1000);
    return () => clearInterval(t);
  }, [until]);
  if (left === null) return null; // 解析失败（理论不可达，后端已校验）
  if (left <= 0) {
    return (
      <p className="font-display text-lg text-[var(--sky)]">
        {dict.maintenance.recovering}
      </p>
    );
  }
  const h = Math.floor(left / 3600);
  const m = Math.floor((left % 3600) / 60);
  const s = left % 60;
  const pad = (n: number) => String(n).padStart(2, "0");
  return (
    <p
      className="font-display text-2xl tabular-nums text-[var(--sky)]"
      role="timer"
      aria-live="off"
    >
      {pad(h)}:{pad(m)}:{pad(s)}
    </p>
  );
}

function untilSeconds(until: string): number | null {
  const t = Date.parse(until);
  if (Number.isNaN(t)) return null;
  return Math.floor((t - Date.now()) / 1000);
}
