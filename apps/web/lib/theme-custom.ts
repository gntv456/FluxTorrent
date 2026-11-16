/** 用户自选主题色（QQ 调色盘「自选颜色」对标，2026-11-14）。
 *
 * 机制：选自选色时 data-theme 落当前主题族的品牌亮底（brand 族落 brand、
 * 山海经族保持 shanhaijing），再从用户所选 #rrggbb 派生令牌组，以
 * **inline style 写在 <html> 上**——inline 优先级高于任何 [data-theme]
 * 样式表块，用户级覆盖站长级 theme_token_*（:root 注入）是合理优先序。
 * 选任何预设档即清除 inline 覆盖。brand 族仅亮色生效（custom 恒落
 * brand 亮底）；山海经族玄墨/宣纸两档都生效（按档位各出一组对比度）。
 *
 * 派生算法必须与 app/layout.tsx ThemeNoFlash 内联脚本里的 ES5 等价实现
 * **逐字同构**（hexToHsl + deriveVars 两函数；沿用 syncThemeCss 双实现的
 * 既有模式）——改任何一处都要同步另一处，测试里有两版同输入同输出的回归。
 *
 * 存储两键：flux-theme="custom"（档位）+ flux-custom-color="#rrggbb"（色值），
 * 都在 localStorage（纯客户端，后端不存，与主题偏好同口径）。 */

import {
  CUSTOM_COLOR_STORAGE_KEY,
  type ThemeChoice,
} from "@/lib/themes";
import { contrast, hslHex } from "@/lib/theme-custom-math";

const HEX_RE = /^#[0-9a-fA-F]{6}$/;

/* ---- HSL 换算（与 layout.tsx 内联脚本 ES5 版同构）------------------- */

/** #rrggbb → [h(0-360), s(0-100), l(0-100)]；非法输入返回 null */
export function hexToHsl(hex: string): [number, number, number] | null {
  if (!HEX_RE.test(hex)) return null;
  const r = parseInt(hex.slice(1, 3), 16) / 255;
  const g = parseInt(hex.slice(3, 5), 16) / 255;
  const b = parseInt(hex.slice(5, 7), 16) / 255;
  const max = Math.max(r, g, b);
  const min = Math.min(r, g, b);
  const l = (max + min) / 2;
  if (max === min) return [0, 0, l * 100]; // 灰阶：色相无意义，走中性蓝
  const d = max - min;
  const s = d / (1 - Math.abs(2 * l - 1));
  let h = 0;
  if (max === r) h = ((g - b) / d) % 6;
  else if (max === g) h = (b - r) / d + 2;
  else h = (r - g) / d + 4;
  h = h * 60;
  if (h < 0) h += 360;
  return [Math.round(h), Math.round(s * 100), Math.round(l * 100)];
}

/** 派生模式：brand = 品牌族（brand/pastel 13 款，整组换肤）；
 *  shanhaijing = 山海经族（玄墨/宣纸两档，只换用户色相相关位——
 *  岩彩底/墨字/金环/金箔渐变/纱层是山海经身份，inline 不碰）。
 *  mode 由调用方按 data-theme 前缀判定（shanhaijing* → shanhaijing）。 */
export type CustomMode = "brand" | "shanhaijing";

/** 数值钳制（ES5 无 Math.clamp，两版同实现） */
function clamp(v: number, lo: number, hi: number): number {
  return Math.min(hi, Math.max(lo, v));
}

/** 从主色 hex 派生品牌令牌组（CSS 变量名 → 值；不写 :root，由调用方决定
 *  落点：客户端 inline style / 内联脚本同款 inline）。对比度口径：
 *  - sky（主交互/实心钮底）：L 钳 38 → 白字 ≥4.5:1；
 *  - skyDeep（标题/文本品牌位）：L 26 → 白底 AA 大裕量；
 *  - soft/wash（浅底）：L 90-97，深字合成后 ≥4.5。
 *
 *  mode="shanhaijing"（2026-11-16）：山海经族专用派生——inline 只覆写
 *  「跟随用户色相」的主干位，玄墨档按暗底、宣纸档按亮底（底值见
 *  PAPER/INKPAGE 常量，摘自 shanhaijing-theme.css 既有档位）。保留位
 *  （不产出=不被覆盖）：表面五阶/墨字五阶/金环 --focus-ring/
 *  金箔渐变 --grad-aurora/--sun/--coral/--mint/纱层墨色系。 */
export function deriveCustomVars(
  hex: string,
  mode: CustomMode = "brand",
  lightMode = false,
): Record<string, string> | null {
  const hsl = hexToHsl(hex);
  if (!hsl) return null;
  const [h, rawS] = hsl;
  // 灰阶/近灰：落回品牌蓝系，避免全站褪色成灰板
  const s = rawS < 12 ? 56 : clamp(rawS, 30, 68);
  const hsl1 = (ss: number, ll: number, a?: number) =>
    a === undefined
      ? `hsl(${h}, ${ss}%, ${ll}%)`
      : `hsla(${h}, ${ss}%, ${ll}%, ${a})`;
  if (mode === "shanhaijing") {
    /* 山海经族：只换用户色相位（键集是 brand 集的子集）。
     * L 值按 WCAG 迭代求达标（≥4.5）——原色明度跨度大（深绿 L32 / 金
     * L52），固定 L 会失守。迭代逻辑与 ES5 版 __shjL 逐字同构（见
     * theme-custom-es5.ts）。文字口径对齐山海经本档（非 brand 白字）：
     * 玄墨档实心钮底配墨字（--text-on-accent 原值）、文字位对暗底；
     * 宣纸档实心钮底配宣纸字、文字位对亮底。 */
    // design-cov-exempt: 山海经主题块锚定值字面量（shanhaijing-theme.css
    // 既有档位原值，平台锚定非新设计色）——下面三行常量均按此豁免
    const PAPER = "#f7f3e8"; // design-cov-exempt: 同上
    const INKPAGE = "#0a0f0d"; // design-cov-exempt: 同上
    const ONACCENT = "#101914"; // design-cov-exempt: 同上
    const target = (base: number, dir: 1 | -1, other: string) => {
      let l = base;
      for (let i = 0; i < 24; i++) {
        const cand = hslHex(h, s, l);
        if (contrast(cand, other) >= 4.5) break;
        l += dir;
        if (l < 4 || l > 96) break;
      }
      return Math.round(l);
    };
    const skyL = lightMode ? target(30, -1, PAPER) : target(78, 1, ONACCENT);
    const deepL = lightMode ? target(26, -1, PAPER) : target(56, 1, INKPAGE);
    const sky = hsl1(s, skyL);
    const deep = hsl1(s, deepL);
    const surf = hsl1(s, 38);
    const skySoft = lightMode ? hsl1(s, 40, 0.12) : hsl1(s, 50, 0.14);
    const indigoSoft = lightMode ? hsl1(s, 40, 0.12) : hsl1(s, 50, 0.16);
    const candySoft = lightMode ? hsl1(s, 40, 0.12) : hsl1(s, 50, 0.14);
    const ringSoft = lightMode ? hsl1(s, 40, 0.24) : hsl1(s, 50, 0.28);
    const shA = lightMode ? 32 : 42;
    const shadow = hsl1(s, shA, lightMode ? 0.22 : 0.28);
    const shadowSoft = hsl1(s, shA, lightMode ? 0.14 : 0.18);
    const rule = hsl1(s, shA, lightMode ? 0.4 : 0.5);
    const line = lightMode ? hsl1(s, 30, 0.34) : hsl1(s, 42, 0.26);
    const orangeBright = lightMode ? hsl1(s, 38) : surf;
    const tideSurf = lightMode ? hsl1(s, 26) : hsl1(s, 38);
    const tideFoam = lightMode ? hsl1(s, 62) : hsl1(s, 66);
    const tideGrid = lightMode ? hsl1(s, 40, 0.1) : hsl1(s, 50, 0.09);
    const tideMist = lightMode ? hsl1(s, 40, 0.14) : hsl1(s, 30, 0.2);
    const tideHaze = lightMode ? hsl1(s, 40, 0.1) : hsl1(s, 28, 0.14);
    return {
      "--sky": sky,
      "--sky-deep": deep,
      "--sky-soft": skySoft,
      "--indigo": deep,
      "--indigo-soft": indigoSoft,
      "--candy": deep,
      "--candy-soft": candySoft,
      "--text-brand": deep,
      "--info": deep,
      "--focus-ring-soft": ringSoft,
      "--accent-shadow": shadow,
      "--accent-shadow-soft": shadowSoft,
      "--accent-rule": rule,
      "--compat-glow": surf,
      "--compat-line": line,
      "--compat-orange": lightMode ? deep : sky,
      "--compat-orange-bright": orangeBright,
      "--compat-orange-dark": deep,
      /* 潮汐桥接：全局结构类消费（山海经块的等效供给被 inline 覆写） */
      "--tide-deep": sky,
      "--tide-sea": sky,
      "--tide-surf": tideSurf,
      "--tide-foam": tideFoam,
      "--tide-signal": deep,
      "--tide-grid": tideGrid,
      "--tide-mist": tideMist,
      "--tide-haze": tideHaze,
    };
  }
  const sky = hsl1(s, 38);
  const skySurf = hsl1(s, 50);
  const skyDeep = hsl1(clamp(s, 32, 58), 26);
  const soft = hsl1(clamp(Math.round(s * 0.62), 20, 46), 92);
  const mist = hsl1(clamp(Math.round(s * 0.55), 18, 42), 90);
  const haze = hsl1(clamp(Math.round(s * 0.45), 14, 36), 96);
  /* 二轮（2026-11-15）：对齐 theme-pastel.css 的 hue 派生口径——卡面带
     5-16% 色相（首版只有页底洗色，用户实测「还是主要白色」）、行纱/chip
     纱改色相纱、边框全线带色相。值域与 pastel 的 color-mix 混白比例
     一致（L 90-97 ≈ 混白 90-95%）。 */
  return {
    "--sky": sky,
    "--sky-deep": skyDeep,
    "--sky-soft": soft,
    "--indigo": skyDeep,
    "--indigo-soft": soft,
    "--text-brand": skyDeep,
    "--info": sky,
    "--compat-glow": skySurf,
    "--compat-glow-veil": hsl1(s, 50, 0.08),
    "--focus-ring": skySurf,
    "--focus-ring-soft": hsl1(s, 50, 0.12),
    "--accent-shadow": hsl1(s, 45, 0.24),
    "--accent-shadow-soft": hsl1(s, 45, 0.16),
    "--accent-rule": hsl1(s, 45, 0.45),
    "--compat-orange": sky,
    "--compat-orange-bright": hsl1(s, 52),
    "--compat-orange-dark": skyDeep,
    "--grad-aurora":
      `linear-gradient(135deg, ${skyDeep} 0%, ${sky} 55%, ` +
      `${hsl1(s, 55)} 100%)`,
    "--grad-rainbow":
      `linear-gradient(90deg, ${skyDeep}, ${sky}, ${hsl1(s, 55)}, ` +
      `${hsl1(s, 66)}, ${hsl1(s, 78)})`,
    "--ribbon": "var(--grad-rainbow)",
    /* 表面系：卡面 5% / raised 8% / hover 12% / cloud 14% / sunken 16%
       （与 pastel 共享块的混色比例逐档一致） */
    "--surface-page": haze,
    "--surface-cloud": mist,
    "--surface-card": hsl1(s, 95),
    "--surface-raised": hsl1(s, 92),
    "--surface-hover": hsl1(clamp(Math.round(s * 0.55), 18, 42), 88),
    "--surface-sunken": hsl1(clamp(Math.round(s * 0.5), 16, 38), 84),
    "--surface-cream": hsl1(clamp(Math.round(s * 0.45), 14, 36), 96),
    "--head-a": soft,
    "--head-b": haze,
    /* 边框系带色相 */
    "--border-soft": mist,
    "--line-soft": hsl1(clamp(Math.round(s * 0.5), 16, 38), 91),
    "--border-deep": hsl1(s, 70),
    "--muted-soft": hsl1(clamp(Math.round(s * 0.5), 16, 38), 90),
    "--compat-line": hsl1(s, 66),
    /* 行纱/chip 纱：色相纱（列表行跟染） */
    "--row-veil": hsl1(clamp(Math.round(s * 0.6), 20, 44), 94, 0.97),
    "--row-veil-soft": hsl1(s, 90, 0.9),
    "--row-veil-mist": hsl1(s, 88, 0.8),
    "--chip-veil": hsl1(clamp(Math.round(s * 0.6), 20, 44), 92, 0.93),
    "--chip-veil-thin": hsl1(s, 90, 0.72),
    /* 促销行高亮：免费行带主色、2x 行带琥珀（与 pastel 同构） */
    "--promo-free-bg": hsl1(s, 88),
    "--promo-x2-bg": hsl1(43, 90),
    /* 潮汐桥接令牌：.btn/.tide-header/.tide-pager 等全局结构类消费它们，
       不覆盖会残留 brand 的潮汐蓝（theme-tide.css 的值） */
    "--tide-sea": skySurf,
    "--tide-deep": skyDeep,
    "--tide-surf": hsl1(s, 55),
    "--tide-foam": hsl1(s, 66),
    "--tide-mist": mist,
    "--tide-haze": hsl1(clamp(Math.round(s * 0.55), 18, 42), 88),
    "--tide-grid": hsl1(s, 50, 0.07),
  };
}

/* ---- 浏览器侧应用/清除（<html> inline style 变量）------------------- */

/** 把派生令牌写进 <html> 的 inline style（inline > 任何 [data-theme] 块）。
 *  上一次应用的键集记在模块级缓存（单页会话内可靠），换色先逐键摘旧再写新，
 *  避免新旧派生键集合不同时残留。传 null = 清除全部。
 *  mode：brand（缺省）| shanhaijing；lightMode：宣纸亮档口径。 */
export function applyCustomStyle(
  hex: string | null,
  mode: CustomMode = "brand",
  lightMode = false,
): void {
  if (typeof document === "undefined") return;
  const root = document.documentElement;
  applyLastKeys.forEach((k) => root.style.removeProperty(k));
  applyLastKeys = [];
  const vars = hex ? deriveCustomVars(hex, mode, lightMode) : null;
  if (!vars) {
    delete root.dataset.customThemed;
    return;
  }
  for (const [name, value] of Object.entries(vars)) {
    root.style.setProperty(name, value);
    applyLastKeys.push(name);
  }
  root.dataset.customThemed = "on";
}
let applyLastKeys: string[] = [];

/** 读存档的自选色（#rrggbb | null） */
export function readCustomColor(): string | null {
  try {
    const v = localStorage.getItem(CUSTOM_COLOR_STORAGE_KEY);
    return v && HEX_RE.test(v) ? v : null;
  } catch {
    return null;
  }
}

/** 持久化自选色（档位 custom 由调用方经 persist 写 flux-theme） */
export function saveCustomColor(hex: string): void {
  try {
    localStorage.setItem(CUSTOM_COLOR_STORAGE_KEY, hex);
  } catch {
    /* 隐私模式存不了就算了，当前会话仍生效 */
  }
}

/** 清自选色（选预设档时调用）：存储 + inline 覆盖一起摘 */
export function clearCustomColor(): void {
  try {
    localStorage.removeItem(CUSTOM_COLOR_STORAGE_KEY);
  } catch {
    /* 同上 */
  }
  applyCustomStyle(null);
}

/** choice 归一化辅助：custom 无存色时退 auto（菜单态与 no-flash 同口径） */
export function normalizeChoice(
  choice: ThemeChoice,
  color: string | null,
): ThemeChoice {
  return choice === "custom" && !color ? "auto" : choice;
}

/** 自选色的派生模式判定（按当前生效主题 id；与 no-flash 内联脚本
 *  的 `t.indexOf('shanhaijing')===0` 前缀判定同口径） */
export function customModeOf(theme: string | undefined): CustomMode {
  return theme && theme.indexOf("shanhaijing") === 0
    ? "shanhaijing"
    : "brand";
}
