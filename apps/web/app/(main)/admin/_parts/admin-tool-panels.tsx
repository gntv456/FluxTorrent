"use client";

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

/** 审计日志列表（完整版；overview 内嵌最近 6 条在首页面板中） */
export function AuditListPanel({ audit }: { audit: AuditRow[] }) {
  const { dict, locale } = useI18n();
  const a = dict.admin as unknown as Record<string, string>;
  return (
    <section className="rounded-[var(--r-lg)] border border-line bg-[var(--surface-card)] p-4 shadow-[var(--shadow-card)]">
      <ul className="flex flex-col divide-y divide-line text-sm">
        {audit.map((row) => (
          <li key={row.id} className="flex items-center justify-between py-2">
            <span className="font-mono text-xs">{row.action}</span>
            <span className="text-xs text-sub">
              {fmt(a.actor, { id: row.actor_id ?? "-" })} ·{" "}
              {new Date(row.created_at).toLocaleString(dateLocale(locale))}
            </span>
          </li>
        ))}
      </ul>
    </section>
  );
}
