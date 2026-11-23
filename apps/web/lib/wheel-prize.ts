import type { JggPrizeView } from "@/lib/games";

/** 转盘档位：与 `/games` 下发的 PrizeRow 同形（单一投影，前端不再造第二份）。 */
export type WheelPrize = JggPrizeView;

/** 该档的魔力等值：物品位用 anchor×qty（后端已折算进 value）。 */
export function prizeValue(p: WheelPrize, ticket: number): number {
  return p.value ?? p.payout * ticket;
}

/** 盘面扇区图标：有配图走 SVG image，没有就按价值档给一枚 emoji。
 *  只是视觉分层，不参与结算；避免给每一档硬编码一张图片资产。 */
export function prizeIcon(p: WheelPrize, ticket: number): string {
  const v = prizeValue(p, ticket);
  if (v <= 0) return "🍀";
  if (v < ticket) return "🔸";
  if (v < ticket * 2) return "🔹";
  if (v < ticket * 3) return "🔷";
  if (v < ticket * 5) return "💠";
  if (v < ticket * 10) return "💎";
  if (v < ticket * 20) return "🏆";
  return "🌟";
}

/** 扇区读数：魔力档 ×N（0 显示 0）；物品档显示图标 × 件数。 */
export function prizeGlyph(p: WheelPrize): string {
  if (p.kind === "item") {
    return p.qty && p.qty > 1
      ? `${p.icon ?? "🎁"}×${p.qty}`
      : (p.icon ?? "🎁");
  }
  return p.payout === 0 ? "0" : `×${trimMult(p.payout)}`;
}

/** 1.0 → "1"，1.5 → "1.5"：盘面读数不留无意义小数。 */
export function trimMult(mult: number): string {
  return mult.toFixed(2).replace(/\.?0+$/, "");
}

/** 抽中档的展示名（2026-11-21 收口）：魔力档用语言无关的「×N 倍率」读数
 *  ——DB label 是站长配的简体串，四语站直出会漏译；物品档没有倍率语义，
 *  仍以 DB label 为准（配合 itemWin 文案展示价值）。 */
export function wheelPrizeName(
  r: { prize: string; mult?: number; kind?: string },
): string {
  if (r.kind !== "item" && typeof r.mult === "number" && r.mult > 0) {
    return `×${trimMult(r.mult)}`;
  }
  return r.prize;
}

/** 奖池概率页每档的副行：倍率 / 折算价值 / 限定道具。
 *  文案由调用方从字典注入，函数本身不落任何中文字面量。 */
export function prizeMeta(
  p: WheelPrize,
  ticket: number,
  L: { item: string; itemQty: string; miss: string; mult: string },
): string {
  if (p.kind === "item") {
    return p.qty && p.qty > 1
      ? L.itemQty.replace("{n}", String(p.qty))
      : L.item;
  }
  if (p.payout === 0) return L.miss;
  return L.mult.replace("{n}", trimMult(p.payout));
}

/** 概率页色彩档：大奖 / 中奖 / 未中（与 token 语义对齐）。 */
export function prizeTone(
  p: WheelPrize,
  ticket: number,
): "gold" | "mint" | "mist" {
  const v = prizeValue(p, ticket);
  if (v >= ticket * 10) return "gold";
  if (v > ticket) return "mint";
  return "mist";
}
