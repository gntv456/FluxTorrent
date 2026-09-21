"use client";

import { useCallback, useEffect, useState } from "react";
import Link from "next/link";
import { api, ApiError } from "@/lib/api-client";
import { useI18n } from "@/i18n/client";

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
  const { dict } = useI18n();
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
        <h2 className="text-base font-bold">举报处理</h2>
        <div className="flex gap-2">
          {(
            [
              ["pending", "待处理"],
              ["handling", "处理中"],
              ["handled", "已处理"],
              ["all", "全部"],
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
        处置备注（随 PM 发给举报人，可空）
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
            <td className="colhead">被举报对象</td>
            <td className="colhead w-24">举报人</td>
            <td className="colhead">理由</td>
            <td className="colhead w-36">时间</td>
            <td className="colhead w-40">处置</td>
          </tr>
        </thead>
        <tbody>
          {(reports ?? []).map((r) => (
            <tr key={r.id} className={r.status === 0 ? "" : "opacity-60"}>
              <td className="num">{r.id}</td>
              <td>
                <span className="rounded-full bg-sky-soft px-2 py-0.5 font-bold">
                  {r.ref_type}
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
                    查看
                  </Link>
                )}
              </td>
              <td>{r.reporter_name ?? `#${r.reporter_id}`}</td>
              <td className="max-w-[280px] break-words">{r.reason}</td>
              <td className="text-sub">
                {new Date(r.created_at).toLocaleString("zh-CN")}
                {r.status === 2 && r.claimed_name && (
                  <p className="text-[11px] text-sky">
                    {r.claimed_name} 处理中
                  </p>
                )}
                {r.status === 1 && r.handled_name && (
                  <p className="text-[11px]">
                    {r.handled_name} 处理于{" "}
                    {r.handled_at
                      ? new Date(r.handled_at).toLocaleString("zh-CN")
                      : "—"}
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
                          }, "已认领，处理中")
                        }
                      >
                        认领
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
                        }, "已处置并通知举报人")
                      }
                    >
                      处置
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
                        }, "已驳回并通知举报人")
                      }
                    >
                      驳回
                    </button>
                    {r.status === 2 && (
                      <button
                        className="ml-1 min-h-[28px] rounded-full border border-line px-3 font-bold text-sub"
                        onClick={() =>
                          guard(async () => {
                            await api.post("/api/v1/admin/reports/release", {
                              report_id: r.id,
                            });
                          }, "已释放回队列")
                        }
                      >
                        释放
                      </button>
                    )}
                  </>
                ) : (
                  <span className="text-sub">已处理</span>
                )}
              </td>
            </tr>
          ))}
          {(reports ?? []).length === 0 && (
            <tr>
              <td colSpan={6} className="py-6 text-center text-sub">
                {reports === null ? "加载失败或无权限" : "暂无举报"}
              </td>
            </tr>
          )}
        </tbody>
      </table>
    </section>
  );
}
