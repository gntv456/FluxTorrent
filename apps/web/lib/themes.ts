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

/** 主题分组（QQ 超级调色盘对标，2026-11-14）：classic=经典四档（不可拆），
 *  fresh/ink/dream = 13 款淡彩渐变配色。UI 选项顺序与面板分组由本字段驱动，
 *  分组显示名走 i18n（themeGroupKey 推导，dict.common）。 */
export type ThemeGroup = "classic" | "fresh" | "ink" | "dream";

/** 分组的固定展示顺序（classic 恒首位）；面板渲染与测试都消费它 */
export const THEME_GROUP_ORDER: ThemeGroup[] = [
  "classic",
  "fresh",
  "ink",
  "dream",
];

/** 主题 id → 分组（注册表 group 字段的扁平视图，避免散落消费方各自 filter） */
export function themeGroupOf(id: ThemeId): ThemeGroup {
  return THEMES[id].group ?? "classic";
}

/** 是否日款（scheme=light）——面板只列日款色卡，夜间孪生由暗色系统
 *  经 sibling 映射自动生效（no-flash 的 PL 列表同款滤法）。 */
export function isLightTheme(id: ThemeId): boolean {
  return THEMES[id].scheme === "light";
}

/** 分组 → i18n 显示名键（dict.common 下）。
 *  fresh → themeGroupFresh；ink → themeGroupInk。 */
export function themeGroupKey(g: ThemeGroup): string {
  const camel = g.replace(/(^|-)([a-z])/g, (_, _d, c: string) =>
    c.toUpperCase(),
  );
  return `themeGroup${camel}`;
}

export interface ThemeDef {
  /** 浏览器界面色（meta theme-color 用，平台字面量） */
  chrome: string;
  /** 明暗族：auto 档解析依据 */
  scheme: "light" | "dark";
  /** 面板分组：缺省 classic（经典四档不带 group 字段，注册表保持瘦） */
  group?: ThemeGroup;
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

  /* ---- QQ 调色盘对标 13 款（2026-11-14）：日间款排首（auto 亮色档仍落
   * brand），夜间款 <id>-night 排末尾（auto 深色档仍落 brand-night）——
   * 手选色卡跟随当前明暗族（resolveTheme 按 sibling 映射），新配色不再把
   * 夜间用户拽到日间（2026-11-16 夜间变体批）。夜间 CSS 在
   * theme-pastel.css 的 -night 块，色值同款色相暗色派生。 ---- */
  // design-cov-exempt: meta theme-color 平台字面量（见文件头说明）
  seablue: { chrome: "#eaf3fd", scheme: "light", group: "fresh" },
  // design-cov-exempt: 同上
  sakura: { chrome: "#fdeef6", scheme: "light", group: "fresh" },
  // design-cov-exempt: 同上
  sprout: { chrome: "#eaf7ee", scheme: "light", group: "fresh" },
  // design-cov-exempt: 同上
  summer: { chrome: "#e8f7e0", scheme: "light", group: "fresh" },
  // design-cov-exempt: 同上
  dragon: { chrome: "#f8ede4", scheme: "light", group: "fresh" },
  // design-cov-exempt: 同上
  gold: { chrome: "#f3ead8", scheme: "light", group: "ink" },
  // design-cov-exempt: 同上
  lilac: { chrome: "#efe9f7", scheme: "light", group: "ink" },
  // design-cov-exempt: 同上
  "bean-green": { chrome: "#e9f0e6", scheme: "light", group: "ink" },
  // design-cov-exempt: 同上
  cinnabar: { chrome: "#f8e9e4", scheme: "light", group: "ink" },
  // design-cov-exempt: 同上
  "orange-pop": { chrome: "#fdefe4", scheme: "light", group: "dream" },
  // design-cov-exempt: 同上
  "calm-green": { chrome: "#e9f6ea", scheme: "light", group: "dream" },
  // design-cov-exempt: 同上
  "blue-mood": { chrome: "#e8f0fc", scheme: "light", group: "dream" },
  // design-cov-exempt: 同上
  "purple-you": { chrome: "#f0ebfa", scheme: "light", group: "dream" },
  /* ---- 13 款夜间变体（2026-11-16）：色相同款，表面走暗色派生。排在
   * 日间款之后 → themeIdByScheme("dark") 仍取 brand-night，auto 行为
   * 不变；仅手选色卡在暗色系统下经 sibling 映射落到本段。 ---- */
  // design-cov-exempt: 同上
  "seablue-night": { chrome: "#10161f", scheme: "dark", group: "fresh" },
  // design-cov-exempt: 同上
  "sakura-night": { chrome: "#1d1418", scheme: "dark", group: "fresh" },
  // design-cov-exempt: 同上
  "sprout-night": { chrome: "#111813", scheme: "dark", group: "fresh" },
  // design-cov-exempt: 同上
  "summer-night": { chrome: "#131a10", scheme: "dark", group: "fresh" },
  // design-cov-exempt: 同上
  "dragon-night": { chrome: "#1a1410", scheme: "dark", group: "fresh" },
  // design-cov-exempt: 同上
  "gold-night": { chrome: "#191510", scheme: "dark", group: "ink" },
  // design-cov-exempt: 同上
  "lilac-night": { chrome: "#161321", scheme: "dark", group: "ink" },
  // design-cov-exempt: 同上
  "bean-green-night": { chrome: "#131814", scheme: "dark", group: "ink" },
  // design-cov-exempt: 同上
  "cinnabar-night": { chrome: "#1a1312", scheme: "dark", group: "ink" },
  // design-cov-exempt: 同上
  "orange-pop-night": { chrome: "#1a1410", scheme: "dark", group: "dream" },
  // design-cov-exempt: 同上
  "calm-green-night": { chrome: "#111812", scheme: "dark", group: "dream" },
  // design-cov-exempt: 同上
  "blue-mood-night": { chrome: "#10161d", scheme: "dark", group: "dream" },
  // design-cov-exempt: 同上
  "purple-you-night": { chrome: "#151220", scheme: "dark", group: "dream" },
};

/** 注册表主题 id 列表（顺序即 UI 选项顺序） */
export const THEME_IDS = Object.keys(THEMES);

export type ThemeId = (typeof THEME_IDS)[number];

/** 用户可选档位：具体主题 / auto（跟随系统）/ custom（用户自选主色，
 *  派生逻辑在 lib/theme-custom.ts；DOM 永远只写具体 ThemeId + inline
 *  覆盖，CSS 不认识 "custom"） */
export type ThemeChoice = ThemeId | "auto" | "custom";

export const THEME_AUTO: ThemeChoice = "auto";

/** 自选颜色的存储键：存 #rrggbb；flux-theme 存 "custom" 时生效 */
export const CUSTOM_COLOR_STORAGE_KEY = "flux-custom-color";

/** localStorage 键（沿用旧键名，存量用户偏好不丢；新增 "auto" 档） */
export const THEME_STORAGE_KEY = "flux-theme";

/** 存储值 → 选择档：非法/缺省值一律归 auto（跟随系统） */
export function parseChoice(v: string | null): ThemeChoice {
  return v && (v === "auto" || v === "custom" || v in THEMES)
    ? (v as ThemeChoice)
    : "auto";
}

/** 明暗族 → 主题 id：auto 解析的唯一实现（服务端/内联脚本同用）。
 *  找不到该族时回退注册表首项，保证永不返回非法 id。 */
export function themeIdByScheme(scheme: "light" | "dark"): ThemeId {
  return THEME_IDS.find((id) => THEMES[id].scheme === scheme) ?? THEME_IDS[0];
}

/** 淡彩色卡的明暗孪生映射（2026-11-16）：`seablue` ⇄ `seablue-night`。
 *  非淡彩族（brand/auto 档）无孪生，返回自身。 */
export function themeSibling(id: ThemeId): ThemeId {
  if (id.endsWith("-night")) {
    const base = id.slice(0, -"-night".length) as ThemeId;
    return base in THEMES ? base : id;
  }
  const night = `${id}-night` as ThemeId;
  return night in THEMES ? night : id;
}

/** 主题 id → i18n 显示名键（dict.common 下）。淡彩夜款与日款同名——
 *  剥 `-night` 后缀后推导（seablue-night → themeSeablue，同一张色卡）。
 *  brand → themeBrand；brand-night → themeBrandNight（夜间是经典档，
 *  有独立名，不剥）。 */
export function themeNameKey(id: ThemeId): string {
  const named = THEMES[id]?.group && id.endsWith("-night")
    ? (id.slice(0, -"-night".length) as ThemeId)
    : id;
  const camel = named.replace(/(^|-)([a-z])/g, (_, _d, c: string) =>
    c.toUpperCase(),
  );
  return `theme${camel}`;
}

/** auto/custom 档解析为具体主题：auto 跟随系统深色偏好；custom 落 brand
 *  （日间亮色底）再由 theme-custom.ts 派生令牌 inline 覆盖。手选淡彩款
 *  跟随系统明暗族落孪生档（暗色系统选 seablue → seablue-night）——
 *  经典档（brand/brand-night/山海经两档）原样通过。需在客户端调用 */
export function resolveTheme(choice: ThemeChoice): ThemeId {
  if (choice === "custom") return "brand";
  if (choice !== "auto") {
    const def = THEMES[choice];
    if (def?.group && def.scheme === "light") {
      const dark =
        typeof window !== "undefined" &&
        window.matchMedia &&
        window.matchMedia("(prefers-color-scheme: dark)").matches;
      if (dark) return themeSibling(choice);
    }
    return choice;
  }
  const dark =
    typeof window !== "undefined" &&
    window.matchMedia &&
    window.matchMedia("(prefers-color-scheme: dark)").matches;
  return themeIdByScheme(dark ? "dark" : "light");
}

/* ---- 山海经主题 CSS 按需注入（2026-11-11 瘦身批）----------------------
 * 字体（184 条 @font-face）与主题令牌只服务山海经两档，已移出 globals.css
 * 全局链，改为运行时注入 public/theme-css/ 下的拷贝件（scripts/gen-theme-css.mjs
 * 生成，文件名带内容 hash → 天然强缓存）。消费同一口径的三处：layout 的
 * ThemeNoFlash 内联脚本（ES5 等价实现，见其注释）、theme-toggle 的 pick()、
 * auto 档跟随系统的重解析。 */

/** <link> 的查找标记（注入/移除都按它找，不碰其他 stylesheet） */
export const THEME_CSS_ATTR = "data-theme-css";

/** 山海经两份 CSS 的 URL（构建期 gen-theme-css.mjs 落 manifest；themeCssLinks
 *  的消费方——内联 no-flash 脚本与 syncThemeCss——都从这两条短字符串取值。
 *  文件名变化即内容 hash 变化，无缓存漂移窗口） */
import themeCssManifestJson from "../public/theme-css/manifest.json";

export const themeCssManifest = themeCssManifestJson as {
  fonts: string;
  theme: string;
};

const SHJ_LINKS = [themeCssManifest.fonts, themeCssManifest.theme];

/** 主题 id → 需要注入的额外 CSS（空 = 无） */
export function themeCssLinks(theme: ThemeId): string[] {
  return theme.startsWith("shanhaijing") ? SHJ_LINKS : [];
}

/** 浏览器侧同步注入/移除山海经 <link>（幂等；SSR 侧 no-op）。 */
export function syncThemeCss(theme: ThemeId): void {
  if (typeof document === "undefined") return;
  const want = themeCssLinks(theme);
  const links = Array.from(
    document.querySelectorAll(`link[${THEME_CSS_ATTR}]`),
  ) as HTMLLinkElement[];
  for (const href of want) {
    if (links.some((l) => l.getAttribute("href") === href)) continue;
    const link = document.createElement("link");
    link.rel = "stylesheet";
    link.href = href;
    link.setAttribute(THEME_CSS_ATTR, "");
    document.head.appendChild(link);
  }
  for (const l of links) {
    if (!want.includes(l.getAttribute("href") ?? "")) l.remove();
  }
}
