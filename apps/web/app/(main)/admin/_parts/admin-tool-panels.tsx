"use client";

import { useMemo, useState } from "react";
import { PANEL_LG } from "@/lib/ui-classes";

/**
 * 管理后台工具页（从 app/(main)/admin/page.tsx 按域拆出）：
 * CheatersPanel 作弊探测、AuditListPanel 审计日志。
 * FreeleechPanel / ClearCachePanel 在 ./admin-freeleech.tsx。
 */

import { useI18n } from "@/i18n/client";
import { dateLocale, fmt } from "@/i18n/config";
import type { AuditRow, CheaterRow } from "./admin-shared";

/** 作弊探测：点击扫描拉取 /admin/cheaters，表格列出可疑上报 */
export function CheatersPanel({
  cheaters,
  onScan,
}: {
  cheaters: CheaterRow[];
  onScan: () => void;
}) {
  const { dict, locale } = useI18n();
  const a = dict.admin as unknown as Record<string, string>;
  return (
    <section className="nexus-detail">
      <h2 className="mb-2 text-base font-bold text-ink">{a.cheatersTitle}</h2>
      <p className="mb-2 text-xs text-sub">{a.cheatersNote}</p>
      <button
        onClick={onScan}
        className="mb-3 min-h-[36px] rounded-full bg-sky px-4 text-xs font-bold text-white"
      >
        {a.cheatersScan}
      </button>
      <div className="baozi-wide-table-scroll">
        <table className="nexus-table">
          <tbody>
            <tr>
              <td className="colhead">{a.cheaterUser}</td>
              <td className="colhead">{a.cheaterTorrent}</td>
              <td className="colhead">{a.cheaterSpeed}</td>
              <td className="colhead">{a.cheaterDelta}</td>
              <td className="colhead">{a.cheaterAt}</td>
            </tr>
            {cheaters.map((row) => (
              <tr key={`${row.user_id}-${row.torrent_id}`}>
                <td className="rowfollow">
                  {row.username} #{row.user_id}
                </td>
                <td className="rowfollow">
                  {row.torrent_id ? (
                    <a href={`/torrent/${row.torrent_id}`}>
                      {row.name ?? `#${row.torrent_id}`}
                    </a>
                  ) : (
                    "—"
                  )}
                </td>
                <td className="rowfollow num">
                  {(row.upspeed / 1024 / 1024).toFixed(1)} MB/s
                </td>
                <td className="rowfollow num">
                  {(row.uploaded_delta / 1024 ** 3).toFixed(2)} GB
                </td>
                <td className="rowfollow text-xs text-sub">
                  {new Date(row.announced_at).toLocaleString(
                    dateLocale(locale),
                  )}
                </td>
              </tr>
            ))}
            {cheaters.length === 0 && (
              <tr>
                <td colSpan={5} className="py-6 text-center text-sub">
                  {a.cheatersEmpty}
                </td>
              </tr>
            )}
          </tbody>
        </table>
      </div>
    </section>
  );
}

/** 审计日志列表（完整版；overview 内嵌最近 6 条在首页面板中）。
 *  一次拉全量（后端无分页参数），前端做搜索过滤 + 分批渲染：
 *  实测 200 行 DOM 高 6600px+，全渲染既卡又难查。 */
export function AuditListPanel({ audit }: { audit: AuditRow[] }) {
  const { dict, locale } = useI18n();
  const a = dict.admin as unknown as Record<string, string>;
  const [q, setQ] = useState("");
  const [limit, setLimit] = useState(100);
  const kw = q.trim().toLowerCase();

  const filtered = useMemo(() => {
    if (!kw) return audit;
    return audit.filter(
      (row) =>
        row.action.toLowerCase().includes(kw) ||
        String(row.actor_id ?? "").includes(kw),
    );
  }, [audit, kw]);
  const shown = filtered.slice(0, limit);

  return (
    <section className={PANEL_LG}>
      <div className="mb-2 flex flex-wrap items-center gap-2">
        <input
          value={q}
          onChange={(e) => {
            setQ(e.target.value);
            setLimit(100);
          }}
          placeholder={a.auditSearchPh}
          className="min-h-[34px] min-w-[200px] flex-1 rounded-[var(--r-sm)]
            border border-line bg-cloud px-3 text-sm outline-none
            focus:border-sky"
        />
        <span className="text-xs text-sub">
          {fmt(a.auditShown, { n: shown.length, total: filtered.length })}
        </span>
      </div>
      <ul className="flex flex-col divide-y divide-line text-sm">
        {shown.map((row) => (
          <li key={row.id} className="flex items-center justify-between py-2">
            <span className="font-mono text-xs">{row.action}</span>
            <span className="text-xs text-sub">
              {fmt(a.actor, { id: row.actor_id ?? "-" })} ·{" "}
              {new Date(row.created_at).toLocaleString(dateLocale(locale))}
            </span>
          </li>
        ))}
        {shown.length === 0 && (
          <li className="py-6 text-center text-sub">{a.queueEmpty}</li>
        )}
      </ul>
      {filtered.length > shown.length && (
        <button
          type="button"
          onClick={() => setLimit((n) => n + 100)}
          className="mt-2 min-h-[34px] w-full rounded-[var(--r-sm)] border
            border-line text-xs font-bold text-sky hover:bg-cloud"
        >
          {a.auditShowMore}
        </button>
      )}
    </section>
  );
}
