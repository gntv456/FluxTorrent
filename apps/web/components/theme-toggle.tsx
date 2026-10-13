"use client";

import { useCallback, useEffect, useState } from "react";
import { Icon } from "@/components/icons";
import { useI18n } from "@/i18n/client";
import {
  THEMES,
  THEME_IDS,
  THEME_STORAGE_KEY,
  parseChoice,
  resolveTheme,
  type ThemeChoice,
  type ThemeId,
} from "@/lib/themes";

/** 主题切换（客户端）：读写 <html data-theme> + localStorage。
 *  初值由 layout 里的 no-flash 内联脚本在首绘前设定（localStorage >
 *  系统偏好），本组件挂载后同步当前值。
 *
 *  W-42 注册表制：档位从二值 toggle 改三选循环
 *  （亮 → 暗 → 跟随系统 → 亮），主题清单/浏览器界面色唯一真源在
 *  lib/themes.ts（THEMES 注册表），本组件只管状态机与 UI；
 *  显示名走 i18n（themeLight/themeDark/themeAuto 三语）。
 *  选 auto 时 data-theme 仍写解析后的具体主题（CSS 不需要认识
 *  "auto"），并监听系统偏好变化实时重解析。
 *  切换瞬间挂 .theme-anim 做约 320ms 的颜色过渡，并同步 PWA
 *  theme-color。 */

function applyChromeColor(theme: ThemeId) {
  let meta = document.querySelector<HTMLMetaElement>(
    'meta[name="theme-color"]',
  );
  if (!meta) {
    meta = document.createElement("meta");
    meta.name = "theme-color";
    document.head.appendChild(meta);
  }
  meta.content = THEMES[theme]?.chrome ?? THEMES.baozi.chrome;
}

function persist(choice: ThemeChoice) {
  try {
    localStorage.setItem(THEME_STORAGE_KEY, choice);
  } catch {
    /* 隐私模式等存不了就算了，当前会话仍生效 */
  }
}

export function ThemeToggle({
  className = "",
  label,
}: {
  className?: string;
  label: string;
}) {
  const { dict } = useI18n();
  const [choice, setChoice] = useState<ThemeChoice>("auto");
  const [mounted, setMounted] = useState(false);

  useEffect(() => {
    const cur = document.documentElement.dataset.theme;
    const saved = parseChoice(localStorage.getItem(THEME_STORAGE_KEY));
    // 有具体主题标记且存的就是非 auto 档 → 保留；否则尊重 auto
    if (saved !== "auto" && (cur === "baozi" || cur === "baozi-night")) {
      setChoice(saved);
    } else {
      setChoice("auto");
    }
    setMounted(true);
  }, []);

  // auto 档跟随系统：偏好翻转时重解析（手动档不受影响）
  useEffect(() => {
    if (choice !== "auto" || !window.matchMedia) return;
    const mq = window.matchMedia("(prefers-color-scheme: dark)");
    const onChange = () => {
      const t = resolveTheme("auto");
      document.documentElement.dataset.theme = t;
      applyChromeColor(t);
    };
    mq.addEventListener?.("change", onChange);
    return () => mq.removeEventListener?.("change", onChange);
  }, [choice]);

  const cycle = useCallback(() => {
    const order: ThemeChoice[] = [...THEME_IDS, "auto"];
    const next = order[(order.indexOf(choice) + 1) % order.length];
    setChoice(next);
    const root = document.documentElement;
    // 平滑过渡：瞬间挂 class，320ms 后移除（避免常驻 transition 开销）
    root.classList.add("theme-anim");
    const theme = resolveTheme(next);
    root.dataset.theme = theme;
    window.setTimeout(() => root.classList.remove("theme-anim"), 320);
    applyChromeColor(theme);
    persist(next);
  }, [choice]);

  // 档位 → 图标/文案（显示名走 i18n，注册表不含文案）
  const names: Record<ThemeChoice, string> = {
    baozi: dict.common.themeLight,
    "baozi-night": dict.common.themeDark,
    auto: dict.common.themeAuto,
  };
  const icon =
    choice === "baozi-night" ? "moon" : choice === "auto" ? "auto" : "sun";
  const full = `${label}：${names[choice]}（${dict.common.themeNext}）`;

  return (
    <button
      type="button"
      onClick={cycle}
      aria-label={full}
      title={full}
      suppressHydrationWarning
      className={`flex h-9 min-h-[44px] w-9 items-center justify-center rounded-[10px] border border-line bg-[var(--surface-raised)] text-base transition-colors hover:border-[var(--baozi-orange)] hover:text-[var(--baozi-orange)] ${className}`}
    >
      {/* mounted 前不渲染图标，避免 SSR/客户端不一致 */}
      {mounted ? (
        <Icon name={icon} size={17} className="leading-none" />
      ) : null}
    </button>
  );
}
