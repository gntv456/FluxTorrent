/** 自选色对比度数学辅助（2026-11-16 山海经模式批拆出；行数门禁）。
 *  消费方 = lib/theme-custom.ts（shanhaijing 模式的 L 迭代求达标）。
 *  **同构纪律**：三函数与 lib/theme-custom-es5.ts 的 __lum/__cr/__h2x
 *  逐字同构（同输入同输出），tests/theme-custom.test.ts 有双版回归。 */

/** #rrggbb 校验（与 lib/theme-custom.ts 的 HEX_RE 同口径） */
const HEX_RE = /^#[0-9a-fA-F]{6}$/;

/** WCAG 相对亮度（hex → 0-1；ES5 版 __lum 同构） */
export function lumOf(hex: string): number | null {
  if (!HEX_RE.test(hex)) return null;
  const ch = [1, 3, 5].map((i) => {
    const c = parseInt(hex.slice(i, i + 2), 16) / 255;
    return c <= 0.03928 ? c / 12.92 : ((c + 0.055) / 1.055) ** 2.4;
  });
  return 0.2126 * ch[0] + 0.7152 * ch[1] + 0.0722 * ch[2];
}

/** WCAG 对比度（hex 对；ES5 版 __cr 同构） */
export function contrast(a: string, b: string): number {
  const l1 = lumOf(a);
  const l2 = lumOf(b);
  if (l1 === null || l2 === null) return 0;
  const hi = Math.max(l1, l2);
  const lo = Math.min(l1, l2);
  return (hi + 0.05) / (lo + 0.05);
}

/** HSL → #rrggbb（与 hexToHsl 的 HLS 公式互逆；ES5 版 __h2x 同构） */
export function hslHex(h: number, s: number, l: number): string {
  const hn = (((h % 360) + 360) % 360) / 360;
  const sn = s / 100;
  const ln = l / 100;
  const c = (1 - Math.abs(2 * ln - 1)) * sn;
  const hh = hn * 6;
  const x = c * (1 - Math.abs((hh % 2) - 1));
  let r = 0;
  let g = 0;
  let b = 0;
  if (hh < 1) [r, g, b] = [c, x, 0];
  else if (hh < 2) [r, g, b] = [x, c, 0];
  else if (hh < 3) [r, g, b] = [0, c, x];
  else if (hh < 4) [r, g, b] = [0, x, c];
  else if (hh < 5) [r, g, b] = [x, 0, c];
  else [r, g, b] = [c, 0, x];
  const m = ln - c / 2;
  const to = (v: number) =>
    Math.round((v + m) * 255)
      .toString(16)
      .padStart(2, "0");
  return "#" + to(r) + to(g) + to(b);
}
