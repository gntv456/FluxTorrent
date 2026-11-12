/** 我的魔力页共享类型（从 page.tsx 按域拆出，300 行门禁）。 */

export interface SparkTiers {
  formula: {
    base_per_hour: number;
    cap_per_hour: number;
    curve_k: number;
    scale: number;
    donor_mult: number;
    official_mult: number;
    note: string;
  };
  /** 官种加成（0488）：单种乘数，站长可在后台关成 1.0 */
  official?: { mult: number; note: string };
  torrent_tier: { name: string; score: number; cond: string }[];
  seed_hours_tier: { name: string; score: number }[];
  volume: { vol_base: number; note: string };
  rarity: { k: number; note: string };
}

export interface SparkInfo {
  balance: number;
  seeding_count: number;
  /** 真正在计收益的颗数（与 seeding_count 的差 = 未计收益） */
  earning_seeding_count?: number;
  /** 未计收益的颗数按原因拆解（互斥，合计 = 总数 - 计收益） */
  seeding_excluded?: {
    port_closed: number;
    unreachable: number;
    stale: number;
  };
  /** 账号级停发（停权 / 作弊处置中）：为真时所有种子都不计 */
  payout_suspended?: boolean;
  /** 0488：本人当前做种的官种数与官种体积（字节） */
  official_seeding_count?: number;
  official_seeding_size?: number;
  hourly_estimate: number;
  /** 收益构成：[档位名, 做种数]（worker 同源口径，两类档位分组） */
  reward_rules?: [string, number][];
  /** 做种计算公式与档位系数（后端 seeding_params() 真值，站长可调） */
  tiers?: SparkTiers;
  /** 完整公式人话版（后端 formula_note） */
  formula_note?: string;
}
