/**
 * 娱乐玩法展示工具（server / client 通用，无 "use client"）。
 * 展示口径一律来自后端下发，不在前端写死 —— 避免出现「前端奖池与后端实现不符」。
 */

export interface ScratchPrize {
  multiplier: number;
  pct: number;
}

export interface JggPrizeView {
  label: string;
  weight_permille: number;
  payout: number;
}

/** 刮刮乐奖池文案：`奖池 0.5x(30%) · 1x(15%) · ...` */
export function scratchPoolText(prizes: ScratchPrize[] | undefined, label: string): string {
  if (!prizes?.length) return "";
  return `${label} ${prizes
    .map((p) => `${p.multiplier}x(${p.pct}%)`)
    .join(" · ")}`;
}

/** 赔率显示：千分比 → 1.9 / 2 / 0.5（去掉多余小数） */
export function fmtMult(mult: number): string {
  return Number.isInteger(mult) ? String(mult) : mult.toFixed(2).replace(/0+$/, "").replace(/\.$/, "");
}
