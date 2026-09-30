"use client";

/**
 * 管理后台总览面板（从 app/(main)/admin/page.tsx 按域拆出）：
 * 待办队列磁贴 + 站点数据 + 服务健康 + 最近操作。数据由管理页注入。
 */

import { useI18n } from "@/i18n/client";
import { dateLocale, fmt } from "@/i18n/config";
const CARD =
  "rounded-[var(--r-md)] border border-line bg-[var(--surface-card)]";
const DOT = "inline-block h-2 w-2 rounded-sm";

import type {
  AppealRow,
  AuditRow,
  Overview,
  PendingTorrent,
  StatsData,
} from "./admin-shared";

export function AdminOverviewPanel({
  ov,
  stats,
  reviews,
  appeals,
  audit,
  onOpen,
}: {
  ov: Overview | null;
  stats: StatsData | null;
  reviews: PendingTorrent[];
  appeals: AppealRow[];
  audit: AuditRow[];
  onOpen: (tool: string) => void;
}) {
  const { dict, locale } = useI18n();
  const a = dict.admin as unknown as Record<string, string>;
  const openAppeals = appeals.filter((x) => x.status === "open").length;
  const uptimeH = stats ? Math.max(1, Math.round(stats.uptime_secs / 3600)) : 0;
  const queue = [
    {
      label: a.pendingReviews,
      v: reviews.length,
      to: "reviews",
      warn: reviews.length > 0,
    },
    {
      label: a.openReports,
      v: ov?.open_reports ?? 0,
      to: "reports",
      warn: (ov?.open_reports ?? 0) > 0,
    },
    {
      label: a.pendingAppeals,
      v: openAppeals,
      to: "appeals",
      warn: openAppeals > 0,
    },
    {
      label: a.cheatersEntry,
      v: null as number | null,
      to: "cheaters",
      warn: false,
    },
  ];
  const site = stats
    ? ([
        [a.users, stats.users],
        [a.torrents, stats.torrents],
        [a.statSeeding, stats.seeding],
        [a.statLeeching, stats.leeching],
        [a.statComments, stats.comments],
        [a.statMessages, stats.messages],
      ] as [string, number][])
    : [];
  return (
    <section className="flex flex-col gap-4">
      <div>
        <h2 className="mb-2 text-sm font-bold text-[var(--text-brand)]">
          {a.queueTitle}
        </h2>
        <div className="grid grid-cols-2 gap-2 md:grid-cols-4">
          {queue.map((s) => (
            <button
              key={s.label}
              onClick={() => onOpen(s.to)}
              title={a.openQueue}
              className={`rounded-[var(--r-md)] border bg-[var(--surface-card)] p-3 text-center shadow-[var(--shadow-card)] transition hover:border-sky ${
                s.warn ? "border-danger/40" : "border-line"
              }`}
            >
              <p className="text-xs text-sub">{s.label}</p>
              <p
                className={`num mt-1 text-2xl ${s.warn ? "text-danger" : "text-sky"}`}
              >
                {s.v ?? "—"}
              </p>
            </button>
          ))}
        </div>
      </div>

      {stats && (
        <div>
          <h2 className="mb-2 text-sm font-bold text-[var(--text-brand)]">
            {a.siteTitle}
          </h2>
          <div className="grid grid-cols-2 gap-2 sm:grid-cols-3 md:grid-cols-6">
            {site.map(([label, v]) => (
              <div
                key={label}
                className="rounded-[var(--r-md)] border border-line bg-[var(--surface-card)] p-3 text-center"
              >
                <p className="text-xs text-sub">{label}</p>
                <p className="num mt-1 text-xl text-ink">
                  {v.toLocaleString()}
                </p>
              </div>
            ))}
          </div>
        </div>
      )}

      {stats && (
        <div className="flex flex-wrap items-center gap-x-5 gap-y-1 rounded-[var(--r-md)] border border-line bg-[var(--surface-card)] px-4 py-2.5 text-xs text-sub">
          <span>
            {a.dbLabel}{" "}
            <b className={stats.db === "up" ? "text-mint" : "text-danger"}>
              {stats.db === "up" ? a.statusOk : a.statusBad}
            </b>
          </span>
          <span>
            Redis{" "}
            <b className={stats.redis === "up" ? "text-mint" : "text-danger"}>
              {stats.redis === "up" ? a.statusOk : a.statusBad}
            </b>
          </span>
          <span>{fmt(a.uptime, { n: uptimeH })}</span>
        </div>
      )}

      {ov && (ov.trend?.length || ov.health) && (
        <div className="grid gap-2 md:grid-cols-2">
          <div className={`${CARD} p-4`}>
            <p className="mb-2 text-xs font-bold text-[var(--text-brand)]">
              {a.trendTitle}
            </p>
            <div className="flex h-24 items-end gap-1">
              {(ov.trend ?? []).map(([d, u, t]) => {
                const max = Math.max(
                  1,
                  ...(ov.trend ?? []).map((x) => Math.max(x[1], x[2])),
                );
                return (
                  <div
                    key={d}
                    className="flex flex-1 flex-col justify-end gap-0.5"
                    title={`${d} · ${a.trendReg} ${u} · ${a.trendPub} ${t}`}
                  >
                    <div
                      className="rounded-t bg-sky/70"
                      style={{ height: `${(u / max) * 70}%` }}
                    />
                    <div
                      className="rounded-t bg-mint/70"
                      style={{ height: `${(t / max) * 70}%` }}
                    />
                  </div>
                );
              })}
            </div>
            <p className="mt-1 text-[10px] text-sub">
              <span className={`mr-2 ${DOT} bg-sky/70`} />
              {a.trendReg}
              <span className={`mx-2 ${DOT} bg-mint/70`} />
              {a.trendPub}
            </p>
          </div>
          <div className="grid grid-cols-2 gap-2">
            {ov.health && (
              <>
                <Stat
                  label={a.healthAliveRate}
                  v={`${ov.health.alive_rate}%`}
                />
                <Stat label={a.healthDead} v={ov.health.dead} />
                <Stat label={a.healthAlive} v={ov.health.alive} />
                <Stat label={a.healthAvgSeeders} v={ov.health.avg_seeders} />
              </>
            )}
            <Stat label={a.wauTitle} v={ov.wau ?? 0} />
          </div>
        </div>
      )}

      <div>
        <h2 className="mb-2 text-sm font-bold text-[var(--text-brand)]">
          {a.recentActions}
        </h2>
        <div className={`${CARD} rounded-[var(--r-lg)] p-4`}>
          <ul className="flex flex-col divide-y divide-line text-sm">
            {audit.slice(0, 6).map((row) => (
              <li
                key={row.id}
                className="flex items-center justify-between py-2"
              >
                <span className="text-xs">
                  {dict.auditLabels[row.action] ?? row.action}
                </span>
                <span className="text-xs text-sub">
                  {fmt(a.actor, { id: row.actor_id ?? "-" })} ·{" "}
                  {new Date(row.created_at).toLocaleString(dateLocale(locale))}
                </span>
              </li>
            ))}
            {audit.length === 0 && (
              <li className="py-4 text-center text-sub">{a.queueEmpty}</li>
            )}
          </ul>
        </div>
      </div>
    </section>
  );
}

function Stat({ label, v }: { label: string; v: string | number }) {
  return (
    <div className={`${CARD} p-3 text-center`}>
      <p className="text-xs text-sub">{label}</p>
      <p className="num mt-1 text-xl text-ink">{v}</p>
    </div>
  );
}
