"use client";

import { BTN_SM_GHOST, INPUT_CARD, INPUT_W32 } from "@/lib/ui-classes";

import { useCallback, useEffect, useState } from "react";
import { api, ApiError } from "@/lib/api-client";
import { useI18n } from "@/i18n/client";
import { dateLocale, fmt } from "@/i18n/config";

/** 第八轮 P2-4：邀请管理（好学站 user/invites 口径）：全站邀请码浏览 */

interface InviteRow {
  id: number;
  inviter: string;
  inviter_id: number;
  code: string;
  status: number;
  used_by: number | null;
  used_by_name: string | null;
  expires_at: string;
}

export function AdminInvites() {
  const { dict, locale } = useI18n();
  const at = dict.adminInvites;
  const c = dict.common;
  const [rows, setRows] = useState<InviteRow[]>([]);
  const [total, setTotal] = useState(0);
  const [uid, setUid] = useState("");
  const [valid, setValid] = useState("");
  const [page, setPage] = useState(1);
  const [msg, setMsg] = useState<string | null>(null);

  const load = useCallback(async () => {
    const p = new URLSearchParams({ page: String(page), per_page: "20" });
    if (uid.trim()) p.set("uid", uid.trim());
    if (valid !== "") p.set("valid", valid);
    try {
      const r = await api.get<{ rows: InviteRow[]; total: number }>(
        `/api/v1/admin/invites?${p}`,
      );
      setRows(r.rows);
      setTotal(r.total);
    } catch (e) {
      setMsg(
        e instanceof ApiError ? e.message : dict.adminInvites.loadFail,
      );
    }
    // eslint-disable-next-line react-hooks/exhaustive-deps
  }, [uid, valid, page]);

  useEffect(() => {
    load();
  }, [load]);

  return (
    <div className="flex flex-col gap-3">
      {msg && (
        <p className="rounded-[var(--r-md)] bg-sky-soft p-3 text-sm text-ink">
          {msg}
        </p>
      )}
      <section className="flex flex-wrap items-end gap-2">
        <label className="flex flex-col gap-1 text-xs">
          {at.fUid}
          <input
            value={uid}
            onChange={(e) => {
              setUid(e.target.value);
              setPage(1);
            }}
            placeholder={at.qAll}
            className={INPUT_W32}
          />
        </label>
        <label className="flex flex-col gap-1 text-xs">
          {at.fStatus}
          <select
            value={valid}
            onChange={(e) => {
              setValid(e.target.value);
              setPage(1);
            }}
            className={INPUT_CARD}
          >
            <option value="">{at.optAll}</option>
            <option value="0">{at.statusLabels["0"]}</option>
            <option value="1">{at.statusLabels["1"]}</option>
            <option value="2">{at.statusLabels["2"]}</option>
            <option value="3">{at.statusLabels["3"]}</option>
          </select>
        </label>
      </section>
      <table className="nexus-table text-xs">
        <thead>
          <tr>
            <td className="colhead">{at.thId}</td>
            <td className="colhead">{at.thInviter}</td>
            <td className="colhead">{at.thCode}</td>
            <td className="colhead">{at.thStatus}</td>
            <td className="colhead">{at.thUsedBy}</td>
            <td className="colhead">{at.thExpires}</td>
          </tr>
        </thead>
        <tbody>
          {rows.map((r) => (
            <tr key={r.id}>
              <td className="num">{r.id}</td>
              <td>
                <a
                  href={`/admin/users/${r.inviter_id}`}
                  className="font-bold text-link"
                >
                  {r.inviter}
                </a>
              </td>
              <td className="font-mono">{r.code.slice(0, 8)}…</td>
              <td>{at.statusLabels[String(r.status)] ?? r.status}</td>
              <td>
                {r.used_by ? (
                  <a href={`/admin/users/${r.used_by}`} className="text-link">
                    {r.used_by_name ?? `#${r.used_by}`}
                  </a>
                ) : (
                  "—"
                )}
              </td>
              <td className="text-sub">
                {new Date(r.expires_at).toLocaleDateString(dateLocale(locale))}
              </td>
            </tr>
          ))}
          {rows.length === 0 && (
            <tr>
              <td colSpan={6} className="py-6 text-center text-sub">
                {at.empty}
              </td>
            </tr>
          )}
        </tbody>
      </table>
      <div className="flex items-center justify-between text-sm text-sub">
        <span>{fmt(c.totalItems, { n: total })}</span>
        <div className="flex gap-2">
          <button
            disabled={page <= 1}
            onClick={() => setPage(page - 1)}
            className={BTN_SM_GHOST}
          >
            {c.prevPage}
          </button>
          <span>{fmt(c.pageX, { n: page })}</span>
          <button
            disabled={rows.length < 20}
            onClick={() => setPage(page + 1)}
            className={BTN_SM_GHOST}
          >
            {c.nextPage}
          </button>
        </div>
      </div>
    </div>
  );
}
