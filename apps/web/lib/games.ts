/**
 * 娱乐玩法展示工具（server / client 通用，无 "use client"）。
 * 展示口径一律来自后端下发，不在前端写死 —— 避免出现「前端奖池与后端实现不符」。
 */

/** 奖池档位的展示形状：两个玩法读同一张表，服务端也只有一份投影，
 *  前端就不该有第二种档位形状（0248 之后刮刮乐也走行表投影）。 */
export interface JggPrizeView {
  label: string;
  weight_permille: number;
  payout: number;
  kind?: string;
  /** 该档的魔力等值（物品位 = anchor × 件数），角标按它排最高值 */
  value?: number;
}

export type ScratchPrize = JggPrizeView;

/** 刮刮乐奖池文案：`奖池 未中奖(45%) · 返本一半(30%) · ...` */
export function scratchPoolText(
  prizes: ScratchPrize[] | undefined,
  label: string,
): string {
  if (!prizes?.length) return "";
  const total = prizes.reduce((a, p) => a + p.weight_permille, 0) || 1;
  return `${label} ${prizes
    .map(
      (p) => `${p.label}(${((p.weight_permille / total) * 100).toFixed(0)}%)`,
    )
    .join(" · ")}`;
}

/** 赔率显示：千分比 → 1.9 / 2 / 0.5（去掉多余小数） */
export function fmtMult(mult: number): string {
  return Number.isInteger(mult)
    ? String(mult)
    : mult.toFixed(2).replace(/0+$/, "").replace(/\.$/, "");
}
