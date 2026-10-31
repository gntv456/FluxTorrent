import type { CSSProperties } from "react";

/**
 * 板块磨砂玻璃（0448）：后台两档 → 容器上的 CSS 变量 + data-glass 开关。
 *
 * 后端 `site_profile.glass` 已夹紧过一次，这里再夹一次：值会直接进 CSS
 * `calc()`，一个越界值（改库/内容包导入都能留下）会让整族表面令牌算成
 * 非法颜色，浏览器静默回退成「板块没有底色」——那种错在界面上看不出来。
 */

/** alpha=100 视为关闭：既不写变量也不挂 data-glass，glass.css 整套不匹配 */
export const GLASS_OFF = 100;
const DEFAULT_ALPHA = 88;
const DEFAULT_BLUR = 12;

export interface GlassConfig {
  alpha?: number | null;
  blur?: number | null;
}

function clamp(v: unknown, dflt: number, lo: number, hi: number): number {
  const n = typeof v === "number" ? v : Number(v);
  return Number.isFinite(n) ? Math.min(hi, Math.max(lo, n)) : dflt;
}

/** 归一后的两档（alpha 0~100、blur 0~30px），越界夹紧、非数回落默认 */
export function glassValues(
  g?: GlassConfig | null,
): { alpha: number; blur: number } {
  return {
    alpha: clamp(g?.alpha, DEFAULT_ALPHA, 0, GLASS_OFF),
    blur: clamp(g?.blur, DEFAULT_BLUR, 0, 30),
  };
}

/** 是否启用（alpha 打满 = 站长明确要实心板块） */
export function glassEnabled(g?: GlassConfig | null): boolean {
  return glassValues(g).alpha < GLASS_OFF;
}

/**
 * 容器内联样式；关闭态返回 null。
 * 自定义属性要走 CSSProperties 的索引签名，故整体断言一次。
 */
export function glassStyle(g?: GlassConfig | null): CSSProperties | null {
  if (!glassEnabled(g)) return null;
  const { alpha, blur } = glassValues(g);
  return {
    "--glass-alpha": String(alpha / 100),
    "--glass-blur": `${blur}px`,
  } as CSSProperties;
}
