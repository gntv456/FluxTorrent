/** 主题注册表（DS-MASTER-01 W-42 / IA-3）：主题的唯一真源。
 *
 * 新增主题 = 在此追加一项 + base.css 补一个 [data-theme] 变量块 +
 * i18n 三语补主题名（dict.common.theme<Id>），别处零改动。
 *
 * 字段说明：
 *  - chrome：浏览器界面色（移动端状态栏 / PWA 标题栏）。meta
 *    theme-color 是平台静态读取的字面量，吃不到 CSS 变量——此处
 *    hex 是平台强制，与 app/layout.tsx viewport themeColor 同口径。
 *  - 显示名不走本表：主题名属 UI 文案，统一从 i18n 字典取
 *    （dict.common.themeLight / themeDark / themeAuto），三语由
 *    i18n 门禁保证不缺。 */

export interface ThemeDef {
  /** 浏览器界面色（meta theme-color 用，平台字面量） */
  chrome: string;
}

export const THEMES: Record<string, ThemeDef> = {
  // design-cov-exempt: meta theme-color 平台字面量（见文件头说明）
  brand: { chrome: "#f5faff" },
  // design-cov-exempt: 同上
  "brand-night": { chrome: "#0f1424" },
};

/** 注册表主题 id 列表（顺序即 UI 选项顺序） */
export const THEME_IDS = Object.keys(THEMES);

export type ThemeId = (typeof THEME_IDS)[number];

/** 用户可选档位：具体主题 或 auto（跟随系统） */
export type ThemeChoice = ThemeId | "auto";

export const THEME_AUTO: ThemeChoice = "auto";

/** localStorage 键（沿用旧键名，存量用户偏好不丢；新增 "auto" 档） */
export const THEME_STORAGE_KEY = "flux-theme";

/** 存储值 → 选择档：非法/缺省值一律归 auto（跟随系统） */
export function parseChoice(v: string | null): ThemeChoice {
  return v && (v === "auto" || v in THEMES) ? (v as ThemeChoice) : "auto";
}

/** auto 档解析：跟随系统深色偏好落到具体主题（需在客户端调用） */
export function resolveTheme(choice: ThemeChoice): ThemeId {
  if (choice !== "auto") return choice;
  return typeof window !== "undefined" &&
    window.matchMedia &&
    window.matchMedia("(prefers-color-scheme: dark)").matches
    ? "brand-night"
    : "brand";
}
