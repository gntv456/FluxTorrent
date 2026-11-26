/** 斗地主：跨牌桌/控制/主件共享的接口形状（对齐后端 view()）。 */

export interface Overview {
  max_bet: number;
  me?: {
    balance: number;
    today_net: number;
    today_plays: number;
    limit_left: number;
  };
}

export interface Seat {
  seat: number;
  role: "landlord" | "farmer" | null;
  count: number;
  cards: number[];
  wildcards?: number[];
  is_ai: boolean;
}

/** 桌面最后一手。kind 是后端 ComboKind 序列化字符串。 */
export interface TableLast {
  seat: number;
  kind: string;
  major: number;
  cards: number[];
}

export interface DzEvent {
  seat: number;
  action: "bid" | "grab" | "discard" | "double" | "play" | "pass";
  score?: number | null;
  cards?: number[];
}

export interface DzGame {
  id: number;
  status: "bidding" | "playing" | "settled";
  phase: "bidding" | "grab" | "discard" | "double" | "playing" | "done";
  turn: number;
  my_turn: boolean;
  stake: number;
  multiplier: number;
  bomb_count: number;
  bids: [number, number][];
  landlord: number | null;
  kitty: number[];
  last: TableLast | null;
  finished: number | null;
  result: "win" | "lose" | null;
  payout: number;
  net: number;
  /** 春天/反春：终局时一方几乎未出牌，倍数翻倍。 */
  spring: boolean;
  /** 明牌：开局选择，倍数 ×2。 */
  reveal: boolean;
  /** 抢地主成功次数。 */
  grabs: number;
  /** 地主弃出的牌。 */
  discards: number[];
  /** 各座位加倍档 0/1/2。 */
  double_levels: number[];
  /** 抢地主投票：[座位, 是否抢]。 */
  grab_votes?: [number, boolean][];
  seats: Seat[];
  events_tail: DzEvent[];
    events?: DzEvent[];
}

/** hint 端点响应：出牌候选或叫分建议。 */
export interface HintResp {
  hint: number[] | null;
  hintBid?: number;
}

export interface StateResp {
  game: DzGame | null;
  last: DzGame | null;
  stake_tiers: number[];
  turn_seconds: number;
  rake_permille: number;
}

/** 牌编码 0..54 → 显示信息（rank 0..12 = 3..2；13 小王 14 大王）。 */
export function cardLabel(card: number): string {
  const r = card >= 52 ? 13 + (card - 52) : Math.floor(card / 4);
  const labels = [
    "3", "4", "5", "6", "7", "8", "9", "10",
    "J", "Q", "K", "A", "2", "joker", "JOKER",
  ];
  return labels[r] ?? "?";
}

export function cardColor(card: number): "red" | "black" | "joker" {
  if (card >= 52) return "joker";
  const suit = card % 4;
  return suit === 1 || suit === 2 ? "red" : "black";
}

/** 花色符号（黑桃/红心/方块/梅花按 suit 位；王牌固定 ★）。 */
export function cardSuit(card: number): string {
  if (card >= 52) return "★";
  return ["♠", "♥", "♦", "♣"][card % 4];
}

/** 按 rank 从大到小排序手牌（与后端降序一致）。 */
export function sortHand(cards: number[]): number[] {
  const rank = (c: number) => (c >= 52 ? 13 + (c - 52) : Math.floor(c / 4));
  return [...cards].sort((a, b) => rank(b) - rank(a));
}
