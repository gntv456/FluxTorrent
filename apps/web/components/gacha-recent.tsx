"use client";

import { useI18n } from "@/i18n/client";

/**
 * 近 30 抽记录（波1 P0）：/gacha/me 的 recent 后端早就在下发，
 * 前端取了从不渲染——这里补上展示面。was_pity 标记直接读后端，
 * 前端不推断保底。
 */

export interface RecentRow {
  seq: number;
  type: string; // output_type: card | shard
  rarity: string | null;
  shards: number;
  wasPity: boolean;
}

export function GachaRecent({
  rows,
  starOf,
}: {
  /** 由调用方把 /gacha/me recent 原始行转成本形状（字段名归一） */
  rows: RecentRow[];
  /** 档位 key → 星数（rarities 表） */
  starOf: (r: string | null) => number;
}) {
  const { dict } = useI18n();
  const tg = dict.games as unknown as Record<string, string>;
  if (!rows.length) {
    return <p className="sw-recent-empty">{tg.gachaRecentEmpty}</p>;
  }
  return (
    <div className="sw-recent-list">
      {rows.map((r, i) => (
        <span
          key={`${r.seq}-${i}`}
          title={r.wasPity ? tg.gachaPityTag : undefined}
          className={`sw-recent-item${
            (r.rarity ? starOf(r.rarity) : 0) >= 5 ? " gold" : ""
          }`}
        >
          {r.type === "shard"
            ? `+${r.shards}`
            : r.type === "miss"
              ? "－"
              : "★".repeat(Math.max(1, r.rarity ? starOf(r.rarity) : 1))}
        </span>
      ))}
    </div>
  );
}
