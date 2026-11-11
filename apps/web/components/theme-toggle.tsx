"use client";

import { useCallback, useEffect, useRef, useState } from "react";
import { Icon } from "@/components/icons";
import { useI18n } from "@/i18n/client";
import {
  THEMES,
  THEME_IDS,
  THEME_STORAGE_KEY,
  parseChoice,
  resolveTheme,
  syncThemeCss,
  themeNameKey,
  type ThemeChoice,
  type ThemeId,
} from "@/lib/themes";
import { catIconSetForTheme } from "@/lib/cat-icon-sets";

/** 主题选择（客户端）：下拉菜单直选，读写 <html data-theme> + localStorage。
 *  初值由 layout 里的 no-flash 内联脚本在首绘前设定（localStorage >
 *  系统偏好），本组件挂载后同步当前值。
 *
 *  W-42 注册表制：选项 = THEMES 顺序 + auto（跟随系统）固定末位，
 *  主题清单/浏览器界面色唯一真源在 lib/themes.ts（THEMES 注册表），
 *  本组件只管状态机与 UI；显示名走 i18n，键名由 themeNameKey(id)
 *  推导（themeBrand/themeBrandNight…），auto 固定 themeAuto。
 *  （2026-11-04 用户体验批：原「档位循环按钮」选山海经平均要点 2-3 下
 *  且不知道循环到哪了，改下拉直选；菜单样式沿用 usermenu 弹层范式。）
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
  meta.content = THEMES[theme]?.chrome ?? THEMES.brand.chrome;
}

function persist(choice: ThemeChoice) {
  try {
    localStorage.setItem(THEME_STORAGE_KEY, choice);
  } catch {
    /* 隐私模式等存不了就算了，当前会话仍生效 */
  }
}

/** 分类图标集按需预取：切换主题时 CatIcon 换目录（aurora ⇄ shanhaijing），
 *  不预取会在切换瞬间闪「加载失败回退块」。触发时机=用户**打开菜单**
 *  （已表露切换意图），一次性预取全部两套候选里**当前页面上已渲染**的
 *  图标（绝不无条件预取全量）；本 Set 防同一 URL 重复插 <link>。 */
const prefetchedIconUrls = new Set<string>();
function prefetchVisibleCatIcons(themes: string[]) {
  const cur = document.documentElement.dataset.theme ?? "";
  for (const t of themes) {
    if (!t || t === cur) continue;
    const set = catIconSetForTheme(t);
    document
      .querySelectorAll<HTMLImageElement>('img[src*="/cat-icons/"]')
      .forEach((img) => {
        const u = img.src.replace(
          /\/cat-icons\/[^/]+\//,
          `/cat-icons/${set}/`,
        );
        if (u === img.src || prefetchedIconUrls.has(u)) return;
        prefetchedIconUrls.add(u);
        const link = document.createElement("link");
        link.rel = "prefetch";
        link.as = "image";
        link.href = u;
        document.head.appendChild(link);
      });
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
  const [open, setOpen] = useState(false);
  const rootRef = useRef<HTMLDivElement>(null);

  useEffect(() => {
    const cur = document.documentElement.dataset.theme;
    const saved = parseChoice(localStorage.getItem(THEME_STORAGE_KEY));
    // 有具体主题标记且存的就是非 auto 档 → 保留；否则尊重 auto
    if (saved !== "auto" && THEME_IDS.includes(cur ?? "")) {
      setChoice(saved);
    } else {
      setChoice("auto");
    }
    setMounted(true);
  }, []);

  // 菜单开合：外点关闭 + Esc 关闭 + 键盘焦点回落触发钮（usermenu 同范式）
  useEffect(() => {
    if (!open) return;
    const onDoc = (e: MouseEvent) => {
      if (rootRef.current && !rootRef.current.contains(e.target as Node))
        setOpen(false);
    };
    const onKey = (e: KeyboardEvent) => {
      if (e.key === "Escape") {
        setOpen(false);
        rootRef.current
          ?.querySelector<HTMLButtonElement>(".thmenu__trigger")
          ?.focus();
      }
    };
    document.addEventListener("mousedown", onDoc);
    document.addEventListener("keydown", onKey);
    return () => {
      document.removeEventListener("mousedown", onDoc);
      document.removeEventListener("keydown", onKey);
    };
  }, [open]);

  // auto 档跟随系统：偏好翻转时重解析（手动档不受影响）
  useEffect(() => {
    if (choice !== "auto" || !window.matchMedia) return;
    const mq = window.matchMedia("(prefers-color-scheme: dark)");
    const onChange = () => {
      const t = resolveTheme("auto");
      document.documentElement.dataset.theme = t;
      syncThemeCss(t);
      applyChromeColor(t);
    };
    mq.addEventListener?.("change", onChange);
    return () => mq.removeEventListener?.("change", onChange);
  }, [choice]);

  /** 直选一档：应用 + 持久化 + 关菜单。复用原 cycle 的过渡处理。 */
  const pick = useCallback((next: ThemeChoice) => {
    setChoice(next);
    const root = document.documentElement;
    // 平滑过渡：瞬间挂 class，320ms 后移除（避免常驻 transition 开销）
    root.classList.add("theme-anim");
    const theme = resolveTheme(next);
    root.dataset.theme = theme;
    syncThemeCss(theme);
    window.setTimeout(() => root.classList.remove("theme-anim"), 320);
    applyChromeColor(theme);
    persist(next);
    setOpen(false);
  }, []);

  // 菜单选项：注册表顺序 + auto 末位
  const options: ThemeChoice[] = [...THEME_IDS, "auto"];
  // 档位 → 图标/文案（显示名走 i18n，注册表不含文案；键名 themeNameKey 推导）
  const dictCommon = dict.common as Record<string, string>;
  const names: Record<ThemeChoice, string> = Object.fromEntries([
    ...THEME_IDS.map((id) => [id, dictCommon[themeNameKey(id)] ?? id]),
    ["auto", dict.common.themeAuto],
  ]);
  const icon =
    choice === "auto"
      ? "auto"
      : THEMES[choice]?.scheme === "dark"
        ? "moon"
        : "sun";
  const current = names[choice] ?? choice;

  // 打开菜单 = 切换意图：预取各候选主题对应的当前页可见分类图标
  const primeAll = useCallback(() => {
    prefetchVisibleCatIcons(options.map(resolveTheme));
    // eslint-disable-next-line react-hooks/exhaustive-deps
  }, [choice]);

  return (
    <div className={`thmenu ${className}`} ref={rootRef}>
      <button
        type="button"
        className="thmenu__trigger"
        aria-haspopup="menu"
        aria-expanded={open}
        aria-label={`${label}：${current}`}
        title={`${label}：${current}`}
        suppressHydrationWarning
        onClick={() => {
          if (!open) primeAll();
          setOpen(!open);
        }}
      >
        {mounted ? <Icon name={icon} size={17} className="leading-none" /> : null}
        <span className="thmenu__caret" aria-hidden />
      </button>
      {open ? (
        <div className="thmenu__drop" role="menu" aria-label={label}>
          {options.map((opt) => {
            const active = opt === choice;
            return (
              <button
                key={opt}
                type="button"
                role="menuitemradio"
                aria-checked={active}
                className={`thmenu__opt${active ? " is-active" : ""}`}
                onClick={() => pick(opt)}
              >
                <Icon
                  name={
                    opt === "auto"
                      ? "auto"
                      : THEMES[opt]?.scheme === "dark"
                        ? "moon"
                        : "sun"
                  }
                  size={15}
                />
                <span className="thmenu__name">{names[opt] ?? opt}</span>
                {active ? (
                  <Icon name="check" size={14} className="thmenu__check" />
                ) : null}
              </button>
            );
          })}
        </div>
      ) : null}
    </div>
  );
}
