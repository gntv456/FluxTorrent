"use client";

import { useEffect, useState } from "react";
import { useI18n } from "@/i18n/client";
import {
  DENSITY_STORAGE_KEY,
  isUiDensity,
  type UiDensity,
} from "@/lib/density";

/** 界面密度切换：写入 localStorage，立即作用到 <html data-density>。
 *  no-flash 脚本负责首绘前恢复，组件只负责交互与状态同步。 */
export function DensityToggle() {
  const { dict } = useI18n();
  const u = dict.uxOpt;
  const [value, setValue] = useState<UiDensity>("comfortable");

  useEffect(() => {
    try {
      const saved = localStorage.getItem(DENSITY_STORAGE_KEY);
      if (isUiDensity(saved)) setValue(saved);
    } catch {
      /* 隐私模式下 localStorage 不可用：退回默认舒适档 */
    }
  }, []);

  function apply(next: UiDensity) {
    setValue(next);
    document.documentElement.setAttribute("data-density", next);
    try {
      localStorage.setItem(DENSITY_STORAGE_KEY, next);
    } catch {
      /* 存储失败不影响当前会话的即时切换 */
    }
  }

  const next = value === "comfortable" ? "compact" : "comfortable";
  const cls = [
    "density-toggle inline-flex min-h-[var(--tap-min)] items-center",
    "rounded-full border border-line bg-[var(--surface-card)] px-3",
    "text-xs font-bold text-sub hover:text-brand",
  ].join(" ");
  return (
    <button
      type="button"
      className={cls}
      aria-label={u.densityLabel}
      title={u.densityLabel}
      aria-pressed={value === "compact"}
      onClick={() => apply(next)}
    >
      <svg
        width="15"
        height="15"
        viewBox="0 0 24 24"
        fill="none"
        stroke="currentColor"
        strokeWidth="2"
        aria-hidden="true"
      >
        <path d="M4 6h16M4 12h16M4 18h10" />
      </svg>
      <span className="ml-1.5">
        {value === "comfortable"
          ? u.densityComfortable
          : u.densityCompact}
      </span>
    </button>
  );
}
