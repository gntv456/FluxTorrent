/** 主题选择器的 DOM 侧工具（2026-11-14 从 theme-toggle.tsx 拆出：行数门禁）。
 *  全部是纯 DOM/存储操作，无 React 依赖；消费方 = theme-toggle.tsx。 */

import {
  THEMES,
  THEME_STORAGE_KEY,
  syncThemeCss,
  type ThemeChoice,
  type ThemeId,
} from "@/lib/themes";
import { catIconSetForTheme } from "@/lib/cat-icon-sets";

/** 同步 PWA meta theme-color（平台读字面量，吃不到 CSS 变量） */
export function applyChromeColor(theme: ThemeId) {
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

/** 持久化档位（隐私模式等存不了就算了，当前会话仍生效） */
export function persistThemeChoice(choice: ThemeChoice) {
  try {
    localStorage.setItem(THEME_STORAGE_KEY, choice);
  } catch {
    /* 同上 */
  }
}

/** 跨实例同步事件名：同页挂多份主题选择器（顶栏 + 抽屉/图鉴侧栏），
 *  各自独立 useState 切换后互不跟随（实测）。storage 事件同 tab 不触发，
 *  两实例共用同一 DOM——pick/pickCustom 后派发本事件，监听方同步档位显示。 */
export const THEME_SYNC_EVENT = "flux-theme-sync";

/** 分类图标集按需预取：切换主题时 CatIcon 换目录（aurora ⇄ shanhaijing），
 *  不预取会在切换瞬间闪「加载失败回退块」。触发时机=用户**打开菜单**
 *  （已表露切换意图），一次性预取候选里**当前页面上已渲染**的图标
 *  （绝不无条件预取全量）；本 Set 防同一 URL 重复插 <link>。 */
const prefetchedIconUrls = new Set<string>();
export function prefetchVisibleCatIcons(themes: string[]) {
  const cur = document.documentElement.dataset.theme ?? "";
  const seen = new Set<string>(themes);
  for (const t of seen) {
    if (!t || t === cur) continue;
    const set = catIconSetForTheme(t);
    document
      .querySelectorAll<HTMLImageElement>('img[src*="/cat-icons/"]')
      .forEach((img) => {
        const u = img.src.replace(/\/cat-icons\/[^/]+\//, `/cat-icons/${set}/`);
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

/** 应用一档（预设主题或 auto）：data-theme + 320ms 过渡 + 按需 CSS +
 *  chrome 色。custom 档的派生覆盖由调用方另行 applyCustomStyle。 */
export function applyThemeDom(theme: ThemeId) {
  const root = document.documentElement;
  root.classList.add("theme-anim");
  root.dataset.theme = theme;
  syncThemeCss(theme);
  window.setTimeout(() => root.classList.remove("theme-anim"), 320);
  applyChromeColor(theme);
}
