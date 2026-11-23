import type { JggPrizeView } from "@/lib/games";

/**
 * 打地鼠档位投影：与 `/games.mole.prizes` 同形（后端 PrizeRow），
 * 前端不再造第二份概率/价值。
 */
export type MolePrize = JggPrizeView;

/** 该档的魔力等值（魔术位 = 票价 × 倍数；物品位后端已折算进 value）。 */
export function moleValue(p: MolePrize, ticket: number): number {
  return p.value ?? p.payout * ticket;
}

/**
 * 冒头地鼠的外观只由「这一档值多少」决定，纯视觉、不参与结算：
 * 5/10/20/50/80/300（票价 100）依次是灰鼠 → 蓝耳 → 粉猪 → 白兔 →
 * 墨镜 → 金鼠；空档是四叶草。改奖池档位时外观跟着价值走，不用配图。
 */
export function moleIcon(value: number, ticket: number): string {
  if (value <= 0) return "🍀";
  const r = ticket > 0 ? value / ticket : 0;
  if (r < 0.08) return "🐹";
  if (r < 0.15) return "🐭";
  if (r < 0.3) return "🐷";
  if (r < 0.6) return "🐰";
  if (r < 1) return "🕶️";
  return "👑";
}

/** 单只地鼠的价值标签：正数带 +，炸弹带 -，空档写 0。 */
export function moleTag(value: number): string {
  if (value === 0) return "0";
  return value > 0 ? `+${value}` : String(value);
}

/** 一局的理论上限（满盘全中）：公示页用它解释「为什么手速再快也回不了本」。 */
export function moleCeiling(
  prizes: MolePrize[],
  ticket: number,
  spawns: number,
): number {
  const totalW = prizes.reduce((s, p) => s + p.weight_permille, 0) || 1;
  const avg =
    prizes.reduce(
      (s, p) => s + (p.weight_permille / totalW) * moleValue(p, ticket),
      0,
    );
  return Math.round(avg * spawns);
}
