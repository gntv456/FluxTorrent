"use client";

/**
 * 游戏子页批（样图 2026-10 补页）：「我的战绩 / 图鉴」整页组件。
 * 战绩走 GET /games/rounds?game=…（对局口径，一局一条）；图鉴走
 * GET /games/collection/{game}（奖池档位 × 命中次数）。两个游戏
 * （钓鱼/抽卡）有专属图鉴端点，不用本件。
 */
import { useEffect, useMemo, useState } from "react";
import Link from "next/link";
import { PANEL_LG } from "@/lib/ui-classes";
import { api } from "@/lib/api-client";
import { useI18n } from "@/i18n/client";

export interface RoundRow {
  game: string;
  bet: number;
  payout: number;
  net: number;
  at: string;
}

interface CollEntry {
  label: string;
  rarity: number;
  count: number;
  mult: number;
  value: number;
}

const RAR_TONE: Record<number, string> = {
  1: "border-[var(--border-deep)]",
  2: "border-line",
  3: "border-sky",
  4: "border-[var(--candy)]",
  5: "border-sun",
};

/** 面板形态：嵌入各游戏主页的「记录/图鉴」tab（无页面头/返回键） */
export function GameRecordsPanel({ game }: { game: string }) {
  return <GameRecords game={game} embedded />;
}

export function GameRecordsPage({ game }: { game: string }) {
  return <GameRecords game={game} />;
}

function GameRecords({
  game,
  embedded = false,
}: {
  game: string;
  embedded?: boolean;
}) {
  const { dict, currency } = useI18n();
  const t = dict.games.sub;
  const [rows, setRows] = useState<RoundRow[] | null>(null);
  const [coll, setColl] = useState<{
    got: number;
    total: number;
    entries: CollEntry[];
  } | null>(null);

  useEffect(() => {
    void api
      .get<RoundRow[]>(`/api/v1/games/rounds?game=${game}&limit=50`)
      .then(setRows)
      .catch(() => setRows([]));
    void api
      .get<{ got: number; total: number; entries: CollEntry[] }>(
        `/api/v1/games/collection/${game}`,
      )
      .then(setColl)
      .catch(() => {});
  }, [game]);

  const stats = useMemo(() => {
    if (!rows) return null;
    const plays = rows.length;
    const wins = rows.filter((r) => r.net > 0).length;
    const best = rows.reduce((m, r) => Math.max(m, r.payout), 0);
    const net = rows.reduce((s, r) => s + r.net, 0);
    return { plays, wins, best, net };
  }, [rows]);

  const gameTitle =
    (dict.games.cards as Record<string, { title: string }>)[game]?.title ??
    game;

  return (
    <div className="flex flex-col gap-4">
      {!embedded && (
        <div className="flex flex-wrap items-center gap-2">
          <Link
            href={`/games/${game}`}
            className="min-h-[36px] rounded-full border border-line px-3 text-xs font-bold text-sub"
          >
            ← {gameTitle}
          </Link>
          <h1 className="font-display text-2xl">{t.recordsTitle}</h1>
        </div>
      )}

      {/* 统计卡（样图②③④ 子页的统计三格） */}
      {stats && (
        <div className="sw-stat-row">
          <div className="sw-stat">
            <div className="sw-stat-lb">{t.statPlays}</div>
            <div className="sw-stat-vl num">{stats.plays}</div>
          </div>
          <div className="sw-stat">
            <div className="sw-stat-lb">{t.statWinRate}</div>
            <div className="sw-stat-vl num">
              {stats.plays > 0
                ? `${Math.round((stats.wins / stats.plays) * 100)}%`
                : "—"}
            </div>
          </div>
          <div className="sw-stat">
            <div className="sw-stat-lb">{t.statBest}</div>
            <div className="sw-stat-vl num gold">{stats.best}</div>
          </div>
          <div className="sw-stat">
            <div className="sw-stat-lb">{t.statNet}</div>
            <div className={`sw-stat-vl num ${stats.net >= 0 ? "green" : ""}`}>
              {stats.net >= 0 ? "+" : ""}
              {stats.net}
            </div>
          </div>
        </div>
      )}

      {/* 记录列表 */}
      <section className={PANEL_LG}>
        <h2 className="mb-3 font-display text-base">{t.recentTitle}</h2>
        {rows === null ? (
          <p className="text-sm text-sub">{dict.common.loading ?? "…"}</p>
        ) : rows.length === 0 ? (
          <p className="text-sm text-sub">{t.empty}</p>
        ) : (
          <div className="flex flex-col gap-1.5">
            {rows.map((r, i) => (
              <div
                key={i}
                className="flex items-center gap-3 rounded-[10px] bg-[var(--surface-raised)] px-3 py-2 text-xs"
              >
                <span className="num shrink-0 text-sub">
                  {new Date(r.at).toLocaleString()}
                </span>
                <span className="ml-auto num shrink-0 text-sub">
                  {t.bet} {r.bet}
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
                  {r.net} {currency}
                </span>
              </div>
            ))}
          </div>
        )}
      </section>

      {/* 图鉴（奖池档位 × 命中）：无图鉴端点的游戏静默隐藏 */}
      {coll && coll.total > 0 && (
        <section className={PANEL_LG}>
          <div className="mb-3 flex items-baseline justify-between gap-2">
            <h2 className="font-display text-base">{t.albumTitle}</h2>
            <span className="num text-xs text-sub">
              {t.albumGot} {coll.got}/{coll.total}
            </span>
          </div>
          <div className="grid grid-cols-3 gap-2 sm:grid-cols-4 lg:grid-cols-6">
            {coll.entries.map((e) => (
              <div
                key={e.label}
                className={`flex flex-col items-center gap-1 rounded-[12px] border-2 bg-[var(--surface-card)] p-2 text-center ${
                  e.count > 0
                    ? (RAR_TONE[e.rarity] ?? "border-line")
                    : "border-dashed border-[var(--border-deep)] opacity-55"
                }`}
              >
                <span className="text-[11px] font-bold leading-tight">
                  {e.count > 0 ? e.label : "???"}
                </span>
                <span className="num text-[10px] text-sub">
                  {e.mult > 0 ? `×${e.mult}` : "物品"} · {t.countLabel}{" "}
                  {e.count}
                </span>
              </div>
            ))}
          </div>
        </section>
      )}
    </div>
  );
}
