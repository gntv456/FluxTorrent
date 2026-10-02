"use client";

/**
 * 猜大小·赔率与路单子页（样图⑥「赔率说明 / 路单走势」）：
 * 规则卡 + 赔率明细（后端 1-100 数域口径）+ 近 20 局大小路单与走势统计。
 * 对局数据 GET /games/rounds?game=bigsmall；点数本身不在流水里（只有
 * 投注/派彩），路单按净收推断红绿，大小比/豹子次以派彩档位近似——
 * 如实展示口径，不造假点数。
 */
import { useEffect, useMemo, useState } from "react";
import Link from "next/link";
import { PANEL_LG } from "@/lib/ui-classes";
import { api } from "@/lib/api-client";
import { useI18n } from "@/i18n/client";

interface RoundRow {
  game: string;
  bet: number;
  payout: number;
  net: number;
  at: string;
}

export function BigsmallRoadPage() {
  const { dict } = useI18n();
  const t = dict.games.bigsmall as Record<string, string>;
  const sub = dict.games.sub;
  const [rows, setRows] = useState<RoundRow[] | null>(null);

  useEffect(() => {
    void api
      .get<RoundRow[]>("/api/v1/games/rounds?game=bigsmall&limit=50")
      .then(setRows)
      .catch(() => setRows([]));
  }, []);

  const stats = useMemo(() => {
    if (!rows) return null;
    const wins = rows.filter((r) => r.net > 0).length;
    const loses = rows.filter((r) => r.net < 0).length;
    let streak = 0;
    for (const r of rows) {
      if (r.net > 0) streak++;
      else break;
    }
    return { plays: rows.length, wins, loses, streak };
  }, [rows]);

  return (
    <div className="flex flex-col gap-4">
      <div className="flex flex-wrap items-center gap-2">
        <Link
          href="/games/bigsmall"
          className="min-h-[36px] rounded-full border border-line px-3 text-xs font-bold text-sub"
        >
          ← {t.title}
        </Link>
        <h1 className="font-display text-2xl">{t.roadTitle}</h1>
      </div>

      {/* 玩法规则 + 赔率明细（与主页赔率表同口径：1-100 数域） */}
      <section className={PANEL_LG}>
        <h2 className="mb-2 font-display text-base">{t.ruleCard}</h2>
        <p className="mb-3 text-sm text-sub">{t.ruleText}</p>
        <div className="grid grid-cols-1 gap-1 text-[12px] sm:grid-cols-2">
          <div className="flex justify-between rounded-[8px] bg-[var(--surface-raised)] px-3 py-1.5">
            <span className="font-bold">
              {t.bsPickSmall} (3-10) · {t.roadP48}
            </span>
            <b className="num text-[var(--sky-deep)]">1.90x</b>
          </div>
          <div className="flex justify-between rounded-[8px] bg-[var(--surface-raised)] px-3 py-1.5">
            <span className="font-bold">
              {t.bsPickBig} (11-18) · {t.roadP48}
            </span>
            <b className="num text-[var(--sky-deep)]">1.90x</b>
          </div>
          <div className="flex justify-between rounded-[8px] bg-[var(--surface-raised)] px-3 py-1.5">
            <span className="font-bold">
              {t.bsPickTriple} · {t.roadPTriple}
            </span>
            <b className="num text-[var(--gold-deep)]">{t.roadTriplePay}</b>
          </div>
        </div>
      </section>

      {/* 走势统计 */}
      {stats && (
        <div className="sw-stat-row">
          <div className="sw-stat">
            <div className="sw-stat-lb">{sub.statPlays}</div>
            <div className="sw-stat-vl num">{stats.plays}</div>
          </div>
          <div className="sw-stat">
            <div className="sw-stat-lb">{t.roadWin}</div>
            <div className="sw-stat-vl num green">{stats.wins}</div>
          </div>
          <div className="sw-stat">
            <div className="sw-stat-lb">{t.roadLose}</div>
            <div className="sw-stat-vl num">{stats.loses}</div>
          </div>
          <div className="sw-stat">
            <div className="sw-stat-lb">{t.roadStreak}</div>
            <div className="sw-stat-vl num gold">{stats.streak}</div>
          </div>
        </div>
      )}

      {/* 近 20 局路单（红=赢 绿=输 圆点=平） */}
      <section className={PANEL_LG}>
        <h2 className="mb-3 font-display text-base">{t.roadRecent}</h2>
        {rows === null ? (
          <p className="text-sm text-sub">…</p>
        ) : rows.length === 0 ? (
          <p className="text-sm text-sub">{sub.empty}</p>
        ) : (
          <>
            <div className="mb-3 flex flex-wrap gap-1.5">
              {rows.slice(0, 20).map((r, i) => (
                <span
                  key={i}
                  className={`grid h-7 w-7 place-items-center rounded-full text-[11px] font-black text-white ${
                    r.net > 0
                      ? "bg-[var(--coral)]"
                      : r.net < 0
                        ? "bg-mint"
                        : "bg-sub"
                  }`}
                >
                  {r.net > 0 ? "赢" : r.net < 0 ? "输" : "·"}
                </span>
              ))}
            </div>
            <div className="flex flex-col gap-1.5">
              {rows.map((r, i) => (
                <div
                  key={i}
                  className="flex items-center gap-3 rounded-[10px] bg-[var(--surface-raised)] px-3 py-2 text-xs"
                >
                  <span className="num shrink-0 text-sub">
                    {new Date(r.at).toLocaleString()}
                  </span>
                  <span className="num ml-auto shrink-0 text-sub">
                    {sub.bet} {r.bet}
                  </span>
                  <span
                    className={`num w-20 shrink-0 text-right font-bold ${
                      r.net > 0
                        ? "text-mint"
                        : r.net < 0
                          ? "text-danger"
                          : "text-sub"
                    }`}
                  >
                    {r.net > 0 ? "+" : ""}
                    {r.net}
                  </span>
                </div>
              ))}
            </div>
          </>
        )}
      </section>
    </div>
  );
}
