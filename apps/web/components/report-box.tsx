"use client";

import { useCallback, useEffect, useState } from "react";
import { api, ApiError } from "@/lib/api-client";
import { useI18n } from "@/i18n/client";

interface ReportRow {
  id: number;
  reporter_id: number;
  ref_type: string;
  ref_id: number;
  reason: string;
  created_at: string;
}

/** 举报信箱（好学站 reports.php 口径）：管理组处理队列 + 全员可提交新举报 */
export function ReportBox() {
  const { dict, locale } = useI18n();
  const t = dict.reportbox;
  const u = dict.usertools;
  const [rows, setRows] = useState<ReportRow[] | null>(null);
  const [noPerm, setNoPerm] = useState(false);

  // 提交表单（原 userbar 弹层表单迁入本页）
  const [refType, setRefType] = useState("torrent");
  const [refId, setRefId] = useState("");
  const [reason, setReason] = useState("");
  const [msg, setMsg] = useState<string | null>(null);
  const [busy, setBusy] = useState(false);

  const load = useCallback(() => {
    api
      .get<ReportRow[]>("/api/v1/admin/reports")
      .then(setRows)
      .catch((e) => {
        if (e instanceof ApiError && e.code === 403) setNoPerm(true);
        else setRows([]);
      });
  }, []);

  useEffect(() => {
    load();
  }, [load]);

  async function resolve(id: number) {
    setBusy(true);
    try {
      await api.post("/api/v1/admin/reports/resolve", { report_id: id });
      setMsg(t.resolveOk);
      load();
    } catch (e) {
      setMsg(e instanceof ApiError ? e.message : dict.common.networkError);
    } finally {
      setBusy(false);
    }
  }

  async function submit() {
    if (!refId || !reason.trim()) return;
    setBusy(true);
    setMsg(null);
    try {
      await api.post("/api/v1/reports", {
        ref_type: refType,
        ref_id: Number(refId),
        reason: reason.trim(),
      });
      setRefId("");
      setReason("");
      setMsg(u.reportOk);
    } catch (e) {
      setMsg(e instanceof ApiError ? e.message : dict.common.networkError);
    } finally {
      setBusy(false);
    }
  }

  const typeLabel = (k: string) =>
    k === "torrent" ? u.rtTorrent
    : k === "user" ? u.rtUser
    : k === "comment" ? u.rtComment
    : k === "subtitle" ? u.rtSubtitle
    : k === "forum" ? u.rtForum
    : k;

  const targetLink = (r: ReportRow) =>
    r.ref_type === "torrent" ? `/torrent/${r.ref_id}`
    : r.ref_type === "user" ? `/users/${r.ref_id}`
    : null;

  const inputCls =
    "min-h-[44px] w-full rounded-[var(--r-sm)] border border-[var(--baozi-line)] bg-[var(--baozi-paper)] px-3 text-sm text-ink outline-none focus:border-[var(--baozi-orange)]";

  const th = "px-3 py-2 text-left text-xs font-bold whitespace-nowrap";
  const td = "px-3 py-2 text-sm align-middle";

  return (
    <section className="flex flex-col gap-4">
      <section className="baozi-panel">
        <header className="baozi-panel__head">
          <h2>
            <span aria-hidden="true">🚩</span> {t.title}
          </h2>
        </header>

        {/* 管理组：待处理队列 */}
        {noPerm ? (
          <p className="funbox__empty">{t.noPerm}</p>
        ) : rows !== null && (
          rows.length === 0 ? (
            <p className="funbox__empty">{t.queueEmpty}</p>
          ) : (
            <div className="overflow-x-auto">
              <table className="w-full border-collapse">
                <thead>
                  <tr className="border-b border-[var(--border-soft)]">
                    <th className={th}>{t.colReporter}</th>
                    <th className={th}>{t.colType}</th>
                    <th className={th}>{t.colTarget}</th>
                    <th className={th}>{t.colReason}</th>
                    <th className={th}>{t.colTime}</th>
                    <th className={th} aria-hidden="true" />
                  </tr>
                </thead>
                <tbody>
                  {rows.map((r) => (
                    <tr key={r.id} className="border-b border-dashed border-[var(--border-soft)]">
                      <td className={td}>#{r.reporter_id}</td>
                      <td className={td}>{typeLabel(r.ref_type)}</td>
                      <td className={td}>
                        {targetLink(r) ? (
                          <a className="hover:underline" href={targetLink(r)!}>
                            {r.ref_type === "user" ? u.rtUser : u.rtTorrent} #{r.ref_id}
                          </a>
                        ) : (
                          `#${r.ref_id}`
                        )}
                      </td>
                      <td className={`${td} max-w-[280px] break-words`}>{r.reason}</td>
                      <td className={`${td} whitespace-nowrap text-xs text-[var(--text-faint)]`}>
                        {new Date(r.created_at).toLocaleString(locale)}
                      </td>
                      <td className={td}>
                        <button
                          type="button"
                          className="min-h-[36px] rounded-full border border-line px-4 text-xs font-bold disabled:opacity-50"
                          disabled={busy}
                          onClick={() => void resolve(r.id)}
                        >
                          {t.resolve}
                        </button>
                      </td>
                    </tr>
                  ))}
                </tbody>
              </table>
            </div>
          )
        )}

        {msg && <p className="funbox__msg">{msg}</p>}
      </section>

      {/* 提交新举报（全员） */}
      <section className="baozi-panel">
        <header className="baozi-panel__head">
          <h2>
            <span aria-hidden="true">📝</span> {t.formTitle}
          </h2>
        </header>
        <form
          className="flex flex-col gap-3 px-4 pb-4"
          onSubmit={(e) => {
            e.preventDefault();
            void submit();
          }}
        >
          <div className="grid gap-3 sm:grid-cols-2">
            <label className="flex flex-col gap-1 text-sm">
              {u.reportType}
              <select className={inputCls} value={refType} onChange={(e) => setRefType(e.target.value)}>
                <option value="torrent">{u.rtTorrent}</option>
                <option value="comment">{u.rtComment}</option>
                <option value="user">{u.rtUser}</option>
                <option value="subtitle">{u.rtSubtitle}</option>
                <option value="forum">{u.rtForum}</option>
              </select>
            </label>
            <label className="flex flex-col gap-1 text-sm">
              {u.reportId}
              <input className={inputCls} inputMode="numeric" value={refId} onChange={(e) => setRefId(e.target.value.replace(/\D/g, ""))} />
            </label>
          </div>
          <label className="flex flex-col gap-1 text-sm">
            {u.reportReason}
            <textarea rows={3} className={inputCls} value={reason} onChange={(e) => setReason(e.target.value)} />
          </label>
          <button type="submit" className="baozi-button self-start disabled:opacity-50" disabled={busy || !refId || !reason.trim()}>
            {u.reportSubmit}
          </button>
        </form>
      </section>
    </section>
  );
}
