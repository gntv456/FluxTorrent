/** 主题注册表（DS-MASTER-01 W-42 / IA-3）：主题的唯一真源。
 *
 * 新增主题 = 在此追加一项 + base.css 补一个 [data-theme] 变量块 +
 * i18n 三语补主题名（键名由 themeNameKey(id) 推导），别处零改动。
 *
 * 字段说明：
 *  - chrome：浏览器界面色（移动端状态栏 / PWA 标题栏）。meta
 *    theme-color 是平台静态读取的字面量，吃不到 CSS 变量——此处
 *    hex 是平台强制，与 app/layout.tsx viewport themeColor 同口径。
 *  - scheme：明暗族。auto（跟随系统）按本字段查具体主题——唯一真源；
 *    禁止再用「注册表位置」或硬编码 id 猜暗色（layout.tsx no-flash
 *    脚本与本文件 resolveTheme 都消费 themeIdByScheme）。
 *  - 显示名不走本表：主题名属 UI 文案，统一从 i18n 字典取
 *    （dict.common[themeNameKey(id)]，如 themeBrand / themeBrandNight；
 *    auto 是模式词不是主题，固定 dict.common.themeAuto），三语齐全由
 *    tests/theme-registry.test.ts 门禁保证。 */

export interface ThemeDef {
  /** 浏览器界面色（meta theme-color 用，平台字面量） */
  chrome: string;
  /** 明暗族：auto 档解析依据 */
  scheme: "light" | "dark";
}

export const THEMES: Record<string, ThemeDef> = {
  // design-cov-exempt: meta theme-color 平台字面量（见文件头说明）
  brand: { chrome: "#f5faff", scheme: "light" },
  // design-cov-exempt: 同上
  "brand-night": { chrome: "#0f1424", scheme: "dark" },
  // design-cov-exempt: 同上。注意必须排在 brand-night 之后：
  // themeIdByScheme("dark") 取首个 dark 族，auto 深色档仍落 brand-night，
  // 山海经只作为用户手选档，不改变既有 auto 行为。
  shanhaijing: { chrome: "#0a0f0d", scheme: "dark" },
  // design-cov-exempt: 山海经亮色档「宣纸」（2026-11-04）：scheme=light
  // 但排在 brand 之后 → auto 亮色档仍落 brand，宣纸只作手选档。
  "shanhaijing-light": { chrome: "#f7f3e8", scheme: "light" },
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

/** 明暗族 → 主题 id：auto 解析的唯一实现（服务端/内联脚本同用）。
 *  找不到该族时回退注册表首项，保证永不返回非法 id。 */
export function themeIdByScheme(scheme: "light" | "dark"): ThemeId {
  return THEME_IDS.find((id) => THEMES[id].scheme === scheme) ?? THEME_IDS[0];
}

/** 主题 id → i18n 显示名键（dict.common 下）。
 *  brand → themeBrand；brand-night → themeBrandNight。 */
export function themeNameKey(id: ThemeId): string {
  const camel = id.replace(/(^|-)([a-z])/g, (_, _d, c: string) =>
    c.toUpperCase(),
  );
  return `theme${camel}`;
}

/** auto 档解析：跟随系统深色偏好落到具体主题（需在客户端调用） */
export function resolveTheme(choice: ThemeChoice): ThemeId {
  if (choice !== "auto") return choice;
  const dark =
    typeof window !== "undefined" &&
    window.matchMedia &&
    window.matchMedia("(prefers-color-scheme: dark)").matches;
  return themeIdByScheme(dark ? "dark" : "light");
}
