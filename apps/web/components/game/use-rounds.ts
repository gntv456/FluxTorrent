"use client";

import { useCallback, useEffect, useState } from "react";
import { api } from "@/lib/api-client";

/**
 * 对局战绩 + 当前连胜（波3 留存层）：`GET /games/rounds` 的统一消费口。
 * 响应 `{rows, streak}`——streak 由服务端按流水算（连续 net>0 局数），
 * 刷新/换设备不丢；替代四页各自的 useState 连胜（此前刷新即丢）。
 */

export interface RoundRow {
  game: string;
  bet: number;
  payout: number;
  net: number;
  at?: string;
  /** 服务端局记录里带的点数（bigsmall 近 5 局走势显大/小用） */
  number?: number;
}

export function useRounds(game: string, limit = 10) {
  const [hist, setHist] = useState<RoundRow[]>([]);
  const [streak, setStreak] = useState(0);

  const load = useCallback(async () => {
    try {
      const r = await api.get<{ rows: RoundRow[]; streak: number }>(
        `/api/v1/games/rounds?game=${game}&limit=${limit}`,
      );
      // 兼容旧形状（裸数组）：容器镜像回滚时不炸
      if (Array.isArray(r)) {
        setHist(r as unknown as RoundRow[]);
      } else {
        setHist(r.rows ?? []);
        setStreak(r.streak ?? 0);
      }
    } catch {
      /* 静默：战绩条失败不影响主玩法 */
    }
  }, [game, limit]);

  useEffect(() => {
    void load();
  }, [load]);

  return { hist, streak, reload: load };
}
