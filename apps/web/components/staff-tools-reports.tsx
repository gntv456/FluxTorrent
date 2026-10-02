"use client";

import { useCallback, useEffect, useState } from "react";
import Link from "next/link";
import { api, ApiError } from "@/lib/api-client";
import { useI18n } from "@/i18n/client";
import { dateLocale, fmt } from "@/i18n/config";

/** 举报处理面板（从 staff-tools.tsx 按域拆出，300 行门禁）：
 *  reports.php 口径——列表 + 处置/驳回 + PM 通知举报人。 */

interface ReportItem {
  id: number;
  reporter_id: number;
  reporter_name: string | null;
  ref_type: string;
  ref_id: number;
  ref_label: string | null;
  reason: string;
  status: number;
  handled_name: string | null;
  handled_at: string | null;
  claimed_name: string | null;
  created_at: string;
}

export function StaffReportsPanel({ flash }: { flash: (m: string) => void }) {
  const { dict, locale } = useI18n();
  const t = dict.adminReports;
  // 举报对象类型 → 人话（与用户侧 report-box 的 typeLabel 同一组键）
  const TYPE_LABEL: Record<string, string> = {
    torrent: dict.usertools.rtTorrent,
    user: dict.usertools.rtUser,
    comment: dict.usertools.rtComment,
    subtitle: dict.usertools.rtSubtitle,
    forum: dict.usertools.rtForum,
  };
  const [reports, setReports] = useState<ReportItem[] | null>(null);
  const [rpStatus, setRpStatus] = useState<
    "pending" | "handling" | "handled" | "all"
  >("pending");
  const [rpNote, setRpNote] = useState("");
  const [busy, setBusy] = useState(false);

  const load = useCallback(async () => {
    api
      .get<ReportItem[]>(`/api/v1/admin/reports?status=${rpStatus}`)
      .then(setReports)
      .catch(() => setReports(null));
  }, [rpStatus]);
  useEffect(() => {
    load();
  }, [load]);

  async function guard(fn: () => Promise<void>, ok: string) {
    setBusy(true);
    try {
      await fn();
      flash(ok);
      await load();
    } catch (e) {
      flash(e instanceof ApiError ? e.message : dict.common.networkError);
    } finally {
      setBusy(false);
    }
  }

  return (
    <section className="baozi-panel p-4">
      <div className="mb-3 flex flex-wrap items-center justify-between gap-2">
        <h2 className="text-base font-bold">{t.title}</h2>
        <div className="flex gap-2">
          {(
            [
              ["pending", t.tabPending],
              ["handling", t.tabHandling],
              ["handled", t.tabHandled],
              ["all", t.tabAll],
            ] as const
          ).map(([k, label]) => (
            <button
              key={k}
              className={`min-h-[32px] rounded-full px-3 text-xs font-bold ${rpStatus === k ? "bg-sky text-white" : "border border-line text-sub"}`}
              onClick={() => setRpStatus(k)}
            >
              {label}
            </button>
          ))}
        </div>
      </div>
      <label className="mb-3 flex items-center gap-2 text-xs text-sub">
        {t.noteLabel}
        <input
          value={rpNote}
          onChange={(e) => setRpNote(e.target.value)}
          className="min-h-[32px] flex-1 rounded-full border border-line bg-cloud px-3 text-sm outline-none focus:border-sky"
        />
      </label>
      <table className="nexus-table text-xs">
        <thead>
          <tr>
            <td className="colhead w-12">#</td>
            <td className="colhead">{t.colTarget}</td>
            <td className="colhead w-24">{t.colReporter}</td>
            <td className="colhead">{t.colReason}</td>
            <td className="colhead w-36">{t.colTime}</td>
            <td className="colhead w-40">{t.colAction}</td>
          </tr>
        </thead>
        <tbody>
          {(reports ?? []).map((r) => (
            <tr key={r.id} className={r.status === 0 ? "" : "opacity-60"}>
              <td className="num">{r.id}</td>
              <td>
                <span className="rounded-full bg-sky-soft px-2 py-0.5 font-bold">
                  {TYPE_LABEL[r.ref_type] ?? r.ref_type}
                </span>
                {r.ref_label ? (
                  <span className="ml-1 text-sub">{r.ref_label}</span>
                ) : (
                  <span className="ml-1 text-sub">#{r.ref_id}</span>
                )}
                {r.ref_type === "torrent" && (
                  <Link
                    href={`/torrents/${r.ref_id}`}
                    className="ml-1 text-sky"
                  >
                    {t.view}
                  </Link>
                )}
              </td>
              <td>{r.reporter_name ?? `#${r.reporter_id}`}</td>
              <td className="max-w-[280px] break-words">{r.reason}</td>
              <td className="text-sub">
                {new Date(r.created_at).toLocaleString(dateLocale(locale))}
                {r.status === 2 && r.claimed_name && (
                  <p className="text-[11px] text-sky">
                    {fmt(t.claimedBy, { name: r.claimed_name })}
                  </p>
                )}
                {r.status === 1 && r.handled_name && (
                  <p className="text-[11px]">
                    {fmt(t.handledBy, {
                      name: r.handled_name,
                      at: r.handled_at
                        ? new Date(r.handled_at).toLocaleString(
                            dateLocale(locale),
                          )
                        : "—",
                    })}
                  </p>
                )}
              </td>
              <td>
                {r.status === 0 || r.status === 2 ? (
                  <>
                    {r.status === 0 && (
                      <button
                        className="min-h-[28px] rounded-full border border-sky px-3 font-bold text-sky"
                        onClick={() =>
                          guard(async () => {
                            await api.post("/api/v1/admin/reports/claim", {
                              report_id: r.id,
                            });
                          }, t.claimed)
                        }
                      >
                        {t.claimBtn}
                      </button>
                    )}
                    <button
                      className="ml-1 min-h-[28px] rounded-full bg-sky px-3 font-bold text-white"
                      onClick={() =>
                        guard(async () => {
                          await api.post("/api/v1/admin/reports/resolve", {
                            report_id: r.id,
                            action: "act",
                            note: rpNote.trim(),
                          });
                          setRpNote("");
                        }, t.acted)
                      }
                    >
                      {t.actBtn}
                    </button>
                    <button
                      className="ml-1 min-h-[28px] rounded-full border border-line px-3 font-bold text-sub"
                      onClick={() =>
                        guard(async () => {
                          await api.post("/api/v1/admin/reports/resolve", {
                            report_id: r.id,
                            action: "dismiss",
                            note: rpNote.trim(),
                          });
                          setRpNote("");
                        }, t.dismissed)
                      }
                    >
                      {t.dismissBtn}
                    </button>
                    {r.status === 2 && (
                      <button
                        className="ml-1 min-h-[28px] rounded-full border border-line px-3 font-bold text-sub"
                        onClick={() =>
                          guard(async () => {
                            await api.post("/api/v1/admin/reports/release", {
                              report_id: r.id,
                            });
                          }, t.released)
                        }
                      >
                        {t.releaseBtn}
                      </button>
                    )}
                  </>
                ) : (
                  <span className="text-sub">{t.done}</span>
                )}
              </td>
            </tr>
          ))}
          {(reports ?? []).length === 0 && (
            <tr>
              <td colSpan={6} className="py-6 text-center text-sub">
                {reports === null ? t.loadFail : t.empty}
              </td>
            </tr>
          )}
        </tbody>
      </table>
    </section>
  );
}
