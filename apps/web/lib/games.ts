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

/** 倒计时文案：ms → 06:12:34。农场市场刷新与地块成熟共用一种读法。 */
export function countdownText(ms: number): string {
  if (ms <= 0) return "00:00:00";
  const h = Math.floor(ms / 3600000);
  const m = Math.floor((ms % 3600000) / 60000);
  const s = Math.floor((ms % 60000) / 1000);
  return (
    `${String(h).padStart(2, "0")}:` +
    `${String(m).padStart(2, "0")}:` +
    `${String(s).padStart(2, "0")}`
  );
}

/** 赔率显示：千分比 → 1.9 / 2 / 0.5（去掉多余小数） */
export function fmtMult(mult: number): string {
  return Number.isInteger(mult)
    ? String(mult)
    : mult.toFixed(2).replace(/0+$/, "").replace(/\.$/, "");
}

/** 农场收获彩蛋的回执（服务端 harvest 响应的 prize 字段）：
 *  魔力档带 extra，物品档带 label，库存耗尽时是回落档。 */
export interface EggPrize {
  label: string;
  kind: string;
  extra?: number;
  fell_back?: string;
}

/** 彩蛋那一句话。空档（倍率 0 的「什么都不加」）不出声 ——
 *  出厂表就是这一档，每次都播报等于把噪声当成交互反馈。 */
export function eggText(
  tf: Record<string, string>,
  p: EggPrize | undefined,
): string {
  if (!p) return "";
  if (p.kind === "fallback") {
    return tf.eggFallback
      .replace("{p}", p.label)
      .replace("{why}", p.fell_back ?? "");
  }
  if (p.kind === "item") return tf.eggItem.replace("{p}", p.label);
  return (p.extra ?? 0) > 0
    ? tf.eggMagic.replace("{n}", String(p.extra ?? 0))
    : "";
}
