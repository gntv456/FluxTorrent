/** 猜大小房间制：跨舞台/控制/主件共享的接口形状。 */

export interface Overview {
  max_bet: number;
  bigsmall?: { win_mult: number; expected_value: number };
  me?: {
    balance: number;
    today_net: number;
    today_plays: number;
    limit_left: number;
  };
}

export interface RoomMine {
  side: "small" | "big";
  amount: number;
  payout: number;
  net: number;
}

export interface RoomLast {
  dice: number[];
  sum: number;
  region: "small" | "big" | "triple";
}

export interface RoomState {
  round: {
    id: number;
    remaining: number;
    window: number;
    total_small: number;
    total_big: number;
  };
  ticket: number;
  win_mult: number;
  mine: RoomMine[];
  my_total: number;
  last: RoomLast | null;
}
