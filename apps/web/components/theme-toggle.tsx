"use client";

import { useCallback, useEffect, useRef, useState } from "react";
import { Icon } from "@/components/icons";
import { useI18n } from "@/i18n/client";
import {
  THEMES,
  THEME_IDS,
  THEME_STORAGE_KEY,
  THEME_GROUP_ORDER,
  isLightTheme,
  parseChoice,
  resolveTheme,
  syncThemeCss,
  themeNameKey,
  themeGroupOf,
  themeGroupKey,
  type ThemeChoice,
  type ThemeGroup,
  type ThemeId,
} from "@/lib/themes";
import {
  applyCustomStyle,
  clearCustomColor,
  customModeOf,
  readCustomColor,
  saveCustomColor,
} from "@/lib/theme-custom";
import { catIconSetForTheme } from "@/lib/cat-icon-sets";
import {
  applyChromeColor,
  applyThemeDom,
  persistThemeChoice as persist,
  prefetchVisibleCatIcons,
  THEME_SYNC_EVENT,
} from "@/lib/theme-dom";
import { PastelGroups } from "@/components/theme-pastel-groups";
import { ClassicGroup } from "@/components/theme-classic-group";

/** 主题选择（客户端）：分组卡片面板（QQ 超级调色盘对标，2026-11-14）。
 *  初值由 layout 里的 no-flash 内联脚本在首绘前设定（localStorage >
 *  系统偏好），本组件挂载后同步当前值。
 *
 *  面板结构：经典组（4 主题 + auto，行式）在前，fresh/ink/dream 三组
 *  色卡网格在后；末位「自选颜色」卡接原生 <input type="color">（custom
 *  档：data-theme 落 brand + 派生令牌 inline 覆盖，见 lib/theme-custom.ts）。
 *  注册表/分组唯一真源在 lib/themes.ts（THEMES + ThemeDef.group），
 *  本组件只管状态机与 UI；显示名/组名走 i18n（themeNameKey/themeGroupKey）。
 *  选 auto 时 data-theme 仍写解析后的具体主题（CSS 不需要认识 "auto"），
 *  并监听系统偏好变化实时重解析。
 *  切换瞬间挂 .theme-anim 做约 320ms 的颜色过渡，并同步 PWA theme-color。 */

/** 主题 DOM 工具（applyChromeColor/persist/预取/同步事件/applyThemeDom）
 *  在 lib/theme-dom.ts，本组件只留状态机与面板 UI（行数门禁拆件）。 */

export function ThemeToggle({
  className = "",
  label,
}: {
  className?: string;
  label: string;
}) {
  const { dict } = useI18n();
  const [choice, setChoice] = useState<ThemeChoice>("auto");
  const [customColor, setCustomColor] = useState<string | null>(null);
  const [mounted, setMounted] = useState(false);
  const [open, setOpen] = useState(false);
  const rootRef = useRef<HTMLDivElement>(null);
  // 应用一档（预设/auto）：DOM 侧封装见 lib/theme-dom.ts
  const applyTheme = useCallback(applyThemeDom, []);

  useEffect(() => {
    const cur = document.documentElement.dataset.theme;
    const saved = parseChoice(localStorage.getItem(THEME_STORAGE_KEY));
    const color = readCustomColor();
    // 有具体主题标记且存的就是非 auto 档 → 保留；custom 无存色退 auto
    if (saved === "custom" && color) {
      setChoice("custom");
      setCustomColor(color);
    } else if (saved !== "auto" && THEME_IDS.includes(cur ?? "")) {
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
      applyTheme(t);
      applyCustomStyle(null); // auto 重解析不需要 custom 覆盖
    };
    mq.addEventListener?.("change", onChange);
    return () => mq.removeEventListener?.("change", onChange);
  }, [choice, applyTheme]);

  // 淡彩手动档跟随系统：偏好翻转时日/夜孪生互切（2026-11-16 夜间变体批）。
  // 经典档（brand/brand-night/山海经）不受系统偏好影响，不挂监听。
  useEffect(() => {
    if (choice === "auto" || choice === "custom" || !window.matchMedia) return;
    const def = THEMES[choice];
    if (!def?.group) return; // 经典档无孪生
    const mq = window.matchMedia("(prefers-color-scheme: dark)");
    const onChange = () => applyTheme(resolveTheme(choice));
    mq.addEventListener?.("change", onChange);
    return () => mq.removeEventListener?.("change", onChange);
  }, [choice, applyTheme]);

  // 跨实例同步：页面挂两份 ThemeToggle（顶栏 + 抽屉/图鉴侧栏），各自独立
  // useState 会在切换后互不跟随（本页实测：顶栏切 seablue，抽屉标签仍留
  // 旧档）。storage 事件同 tab 不触发，但两实例共用同一 DOM——直接监听
  // flux-theme 键的自定义事件（pick/pickCustom 派发）互相同步档位显示。
  useEffect(() => {
    const onSync = (e: Event) => {
      const detail = (e as CustomEvent<ThemeChoice>).detail;
      if (detail && detail !== choice) setChoice(detail);
    };
    window.addEventListener(THEME_SYNC_EVENT, onSync);
    return () => window.removeEventListener(THEME_SYNC_EVENT, onSync);
  }, [choice]);

  /** 直选预设档：应用 + 清 custom 覆盖 + 持久化 + 关面板 */
  const pick = useCallback(
    (next: ThemeChoice) => {
      setChoice(next);
      applyTheme(resolveTheme(next));
      if (next !== "custom") {
        clearCustomColor();
        setCustomColor(null);
      }
      persist(next);
      window.dispatchEvent(
        new CustomEvent(THEME_SYNC_EVENT, { detail: next }),
      );
      setOpen(false);
    },
    [applyTheme],
  );

  /** 自选颜色提交（原生取色器 onChange；停止前不关面板便于微调）。
   *  山海经族保持山海经档（玄墨/宣纸按当前档派生），brand 族落 brand。 */
  const pickCustom = useCallback(
    (hex: string) => {
      const up = hex.toUpperCase();
      const cur = document.documentElement.dataset.theme ?? "";
      const isShj = cur.indexOf("shanhaijing") === 0;
      setChoice("custom");
      setCustomColor(up);
      saveCustomColor(up);
      persist("custom");
      applyTheme(isShj ? "shanhaijing" : "brand");
      applyCustomStyle(
        up,
        isShj ? "shanhaijing" : "brand",
        cur === "shanhaijing-light",
      );
      window.dispatchEvent(
        new CustomEvent(THEME_SYNC_EVENT, { detail: "custom" }),
      );
    },
    [applyTheme],
  );

  // 面板分组：淡彩组只列日款（isLightTheme，夜间孪生由暗色系统经
  // sibling 映射自动生效）；经典组不过滤——brand-night/山海经是独立
  // 主题档非淡彩孪生，用户靠它们手选暗色（滤掉=抹出面板）。
  const groups: ThemeGroup[] = THEME_GROUP_ORDER;
  const byGroup = useCallback((g: ThemeGroup) => {
    const ids = THEME_IDS.filter((id) => themeGroupOf(id) === g);
    return g === "classic" ? ids : ids.filter(isLightTheme);
  }, []);
  const dictCommon = dict.common as Record<string, string>;
  const nameOf = useCallback(
    (id: ThemeId) => dictCommon[themeNameKey(id)] ?? id,
    [dictCommon],
  );
  // active 态按明暗家族判断：面板只有日款色卡，暗色系统下手选落的是
  // -night 孪生（choice 存日款 id、data-theme 是 -night）——孪生命中即亮勾
  const currentTheme = mounted
    ? (document.documentElement.dataset.theme as ThemeId | undefined)
    : undefined;
  const isFamilyActive = useCallback(
    (id: ThemeId) =>
      choice === id ||
      choice === `${id}-night` ||
      (choice !== "auto" && choice !== "custom" && currentTheme === id) ||
      (choice !== "auto" &&
        choice !== "custom" &&
        currentTheme === `${id}-night`),
    [choice, currentTheme],
  );
  const autoName = dict.common.themeAuto;
  const customName =
    (dict.common as Record<string, string>).themeCustom ?? "Custom";
  const groupName = (g: ThemeGroup) =>
    (dictCommon[themeGroupKey(g)] as string | undefined) ?? g;
  const currentLabel =
    choice === "custom"
      ? customName
      : choice === "auto"
        ? autoName
        : nameOf(choice);
  const triggerIcon =
    choice === "custom"
      ? "palette"
      : choice === "auto"
        ? "auto"
        : // 实际生效档（暗色系统下淡彩手选落 -night 孪生）
          (currentTheme && THEMES[currentTheme]?.scheme === "dark")
          ? "moon"
          : "sun";

  // 打开菜单 = 切换意图：预取各候选主题对应的当前页可见分类图标
  const primeAll = useCallback(() => {
    prefetchVisibleCatIcons(THEME_IDS.map(resolveTheme));
  }, []);

  const classicIds = byGroup("classic");

  return (
    <div className={`thmenu ${className}`} ref={rootRef}>
      <button
        type="button"
        className="thmenu__trigger"
        aria-haspopup="menu"
        aria-expanded={open}
        aria-label={`${label}：${currentLabel}`}
        title={`${label}：${currentLabel}`}
        suppressHydrationWarning
        onClick={() => {
          if (!open) primeAll();
          setOpen(!open);
        }}
      >
        {mounted ? (
          <Icon name={triggerIcon} size={17} className="leading-none" />
        ) : null}
        <span className="thmenu__caret" aria-hidden />
      </button>
      {open ? (
        <div
          className="thmenu__drop thmenu__drop--panel"
          role="menu"
          aria-label={label}
        >
          {/* 经典组：行式（4 档 + auto，沿用原下拉选项契约） */}
          <ClassicGroup
            ids={classicIds}
            choice={choice}
            nameOf={nameOf}
            autoName={autoName}
            classicLabel={groupName("classic")}
            onPick={pick}
          />

          {/* 淡彩三组：色卡网格（fresh 5 / ink 4 / dream 4）+ 自选颜色卡 */}
          <PastelGroups
            groups={groups}
            byGroup={byGroup}
            choice={choice}
            customColor={customColor}
            nameOf={nameOf}
            groupName={groupName}
            customLabel={customName}
            isActive={isFamilyActive}
            onPick={pick}
            onPickCustom={pickCustom}
          />
        </div>
      ) : null}
    </div>
  );
}
