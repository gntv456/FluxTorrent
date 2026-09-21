"use client";

/**
 * 管理后台总览面板（从 app/(main)/admin/page.tsx 按域拆出）：
 * 待办队列磁贴 + 站点数据 + 服务健康 + 最近操作。数据由管理页注入。
 */

import { useI18n } from "@/i18n/client";
import { dateLocale, fmt } from "@/i18n/config";
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
      label: "待处理申诉",
      v: openAppeals,
      to: "appeals",
      warn: openAppeals > 0,
    },
    {
      label: "作弊探测",
      v: null as number | null,
      to: "cheaters",
      warn: false,
    },
  ];
  const site = stats
    ? ([
        [a.users, stats.users],
        ["种子", stats.torrents],
        ["做种中", stats.seeding],
        ["下载中", stats.leeching],
        ["评论", stats.comments],
        ["站内信", stats.messages],
      ] as [string, number][])
    : [];
  return (
    <section className="flex flex-col gap-4">
      <div>
        <h2 className="mb-2 text-sm font-bold text-[var(--text-brand)]">
          待办队列
        </h2>
        <div className="grid grid-cols-2 gap-2 md:grid-cols-4">
          {queue.map((s) => (
            <button
              key={s.label}
              onClick={() => onOpen(s.to)}
              title="点击打开对应队列"
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
            站点数据
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
            数据库{" "}
            <b className={stats.db === "up" ? "text-mint" : "text-danger"}>
              {stats.db === "up" ? "正常" : "异常"}
            </b>
          </span>
          <span>
            Redis{" "}
            <b className={stats.redis === "up" ? "text-mint" : "text-danger"}>
              {stats.redis === "up" ? "正常" : "异常"}
            </b>
          </span>
          <span>API 已运行 {uptimeH} 小时</span>
        </div>
      )}

      <div>
        <h2 className="mb-2 text-sm font-bold text-[var(--text-brand)]">
          {a.recentActions ?? "最近操作"}
        </h2>
        <div className="rounded-[var(--r-lg)] border border-line bg-[var(--surface-card)] p-4">
          <ul className="flex flex-col divide-y divide-line text-sm">
            {audit.slice(0, 6).map((row) => (
              <li
                key={row.id}
                className="flex items-center justify-between py-2"
              >
                <span className="font-mono text-xs">{row.action}</span>
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
