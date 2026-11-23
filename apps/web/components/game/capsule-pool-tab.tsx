"use client";

import { GameRules } from "@/components/game/game-rules";
import type { SlotPrize } from "@/components/game/slot-machine";

/** 老虎机 / 水果机侧栏「奖池」分区（布局优化 2026-11-23）。
 *  奖池概率 chips + 稀有度占比条 + 个人收集进度——两页此前各写一份
 *  逐字相同的代码，抽成共用件；内容从控制卡移入侧栏后控制卡只剩下注
 *  按钮。文案由调用方从各自字典段（slot / fruit）传入，本件不翻字典。 */
export function CapsulePoolTab({
  game,
  prizes,
  ticket,
  collect,
  collectLabel,
}: {
  game: "slot" | "fruit";
  prizes: SlotPrize[];
  ticket: number;
  /** 个人图鉴：按稀有度分桶的已收集 / 总数 / 命中次数 */
  collect: {
    tiers: { tier: string; got: number; total: number; hits: number }[];
  } | null;
  /** 「已集齐 {a}/{b}」模板（dict.games.<slot|fruit>.collectGot） */
  collectLabel: string;
}) {
  // 稀有度占比：同稀有度的权重求和 / 总权重
  const buckets: Record<number, number> = {};
  for (const p of prizes) {
    const r = p.rarity ?? 1;
    buckets[r] = (buckets[r] ?? 0) + p.weight_permille;
  }
  const total = Object.values(buckets).reduce((a, b) => a + b, 0) || 1;
  const meters = [2, 3, 4].map((r) => ({
    r,
    pct: Math.round(((buckets[r] ?? 0) / total) * 100),
  }));

  return (
    <>
      <div className="flex flex-wrap gap-1.5">
        {prizes.map((p, i) => {
          const v = p.value ?? p.payout * ticket;
          return (
            <span
              key={i}
              className={`rounded-full px-2 py-0.5 text-xs font-bold ${
                v >= ticket * 10
                  ? "bg-[var(--tone-warn-soft)] text-[var(--warning)]"
                  : v > ticket
                    ? "bg-[var(--tone-success-soft)] text-[var(--mint)]"
                    : "bg-[var(--surface-sunken)] text-sub"
              }`}
            >
              {p.label} {(p.weight_permille / 10).toFixed(1)}%
            </span>
          );
        })}
      </div>
      <div className="sw-cap-meters mt-3">
        {meters.map((m) => (
          <div key={m.r} className="sw-cap-meter">
            <span className="sw-cap-r num">
              {m.r >= 4 ? "SSR" : m.r === 3 ? "SR" : "R"}
            </span>
            <span className="sw-cap-track">
              <i style={{ width: `${m.pct}%` }} />
            </span>
            <span className="num sw-cap-pct">{m.pct}%</span>
          </div>
        ))}
      </div>
      {collect && (
        <div className="sw-cap-collect">
          {collect.tiers.map((tr) => (
            <div key={tr.tier} className="sw-cap-crow">
              <b className="num sw-cap-ctier">{tr.tier}</b>
              <span className="num">
                {collectLabel
                  .replace("{a}", String(tr.got))
                  .replace("{b}", String(tr.total))}
              </span>
              {tr.hits > 0 && (
                <span className="num sw-cap-chits">×{tr.hits}</span>
              )}
            </div>
          ))}
        </div>
      )}
      <GameRules game={game} />
    </>
  );
}
