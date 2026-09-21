"use client";

import { useCallback, useEffect, useState } from "react";

/** 主题切换（客户端）：读写 <html data-theme> + localStorage。
 *  初值由 layout 里的 no-flash 内联脚本在首绘前设定（localStorage > 系统偏好），
 *  本组件挂载后同步当前值，点击在 baozi（日间）/ baozi-night（夜间）间切换。
 *  切换瞬间挂 .theme-anim 做约 320ms 的颜色过渡，并同步 PWA theme-color。 */

const STORAGE_KEY = "flux-theme";

export type Theme = "baozi" | "baozi-night";

/** 主题对应的浏览器界面色（移动端状态栏 / PWA 标题栏） */
const THEME_CHROME: Record<Theme, string> = {
  baozi: "#f5faff",
  "baozi-night": "#0f1424",
};

function applyChromeColor(theme: Theme) {
  let meta = document.querySelector<HTMLMetaElement>(
    'meta[name="theme-color"]',
  );
  if (!meta) {
    meta = document.createElement("meta");
    meta.name = "theme-color";
    document.head.appendChild(meta);
  }
  meta.content = THEME_CHROME[theme];
}

export function ThemeToggle({ className = "" }: { className?: string }) {
  const [theme, setTheme] = useState<Theme>("baozi");
  const [mounted, setMounted] = useState(false);

  useEffect(() => {
    const cur = document.documentElement.dataset.theme;
    if (cur === "baozi-night" || cur === "baozi") setTheme(cur);
    setMounted(true);
  }, []);

  const toggle = useCallback(() => {
    const next: Theme = theme === "baozi" ? "baozi-night" : "baozi";
    setTheme(next);
    const root = document.documentElement;
    // 平滑过渡：瞬间挂 class，320ms 后移除（避免常驻 transition 的渲染开销）
    root.classList.add("theme-anim");
    root.dataset.theme = next;
    window.setTimeout(() => root.classList.remove("theme-anim"), 320);
    applyChromeColor(next);
    try {
      localStorage.setItem(STORAGE_KEY, next);
    } catch {
      /* 隐私模式等存不了就算了，当前会话仍生效 */
    }
  }, [theme]);

  return (
    <button
      type="button"
      onClick={toggle}
      aria-label="切换日间/夜间主题 / Toggle theme"
      title="切换主题"
      suppressHydrationWarning
      className={`flex h-9 min-h-[44px] w-9 items-center justify-center rounded-[10px] border border-line bg-[var(--surface-raised)] text-base transition-colors hover:border-[var(--baozi-orange)] hover:text-[var(--baozi-orange)] ${className}`}
    >
      {/* mounted 前不渲染图标，避免 SSR/客户端不一致 */}
      <span aria-hidden className="leading-none">
        {mounted && theme === "baozi-night" ? "🌙" : "☀️"}
      </span>
    </button>
  );
}
