"use client";

import { BTN_SM_GHOST, INPUT_CARD, INPUT_W32 } from "@/lib/ui-classes";

import { useCallback, useEffect, useState } from "react";
import { api, ApiError } from "@/lib/api-client";
import { useI18n } from "@/i18n/client";
import { dateLocale, fmt } from "@/i18n/config";

/** 第八轮 P2-4：邀请管理（好学站 user/invites 口径）：全站邀请码浏览。
 *  0204：邮箱/被邀请人筛选 + 直发码 + 作废（管理端从只读升级为可操作）。 */

interface InviteRow {
  id: number;
  inviter: string;
  inviter_id: number;
  code: string;
  status: number;
  used_by: number | null;
  used_by_name: string | null;
  expires_at: string;
  email?: string | null;
}

export function AdminInvites() {
  const { dict, locale } = useI18n();
  const at = dict.adminInvites;
  const c = dict.common;
  const [rows, setRows] = useState<InviteRow[]>([]);
  const [total, setTotal] = useState(0);
  const [uid, setUid] = useState("");
  const [valid, setValid] = useState("");
  const [email, setEmail] = useState("");
  const [usedByName, setUsedByName] = useState("");
  const [page, setPage] = useState(1);
  const [msg, setMsg] = useState<string | null>(null);
  // 直发表单（0204）
  const [issueUid, setIssueUid] = useState("");
  const [issueCount, setIssueCount] = useState("1");
  const [issueDays, setIssueDays] = useState("");
  const [busy, setBusy] = useState(false);
  const [issuedCodes, setIssuedCodes] = useState<string[] | null>(null);

  const load = useCallback(async () => {
    const p = new URLSearchParams({ page: String(page), per_page: "20" });
    if (uid.trim()) p.set("uid", uid.trim());
    if (valid !== "") p.set("valid", valid);
    if (email.trim()) p.set("email", email.trim());
    if (usedByName.trim()) p.set("used_by_name", usedByName.trim());
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
  }, [uid, valid, email, usedByName, page]);

  useEffect(() => {
    load();
  }, [load]);

  async function issue() {
    const n = Number(issueCount);
    if (!Number.isFinite(n) || !(1 <= n && n <= 50)) return;
    setBusy(true);
    setMsg(null);
    setIssuedCodes(null);
    try {
      const r = await api.post<{
        codes: { code: string }[];
        days: number;
      }>("/api/v1/admin/invites", {
        user_id: Number(issueUid),
        count: n,
        ...(issueDays ? { days: Number(issueDays) } : {}),
      });
      setIssuedCodes(r.codes.map((x) => x.code));
      setMsg(fmt(at.issueOk, { n: r.codes.length, days: r.days }));
      await load();
    } catch (e) {
      setMsg(e instanceof ApiError ? e.message : dict.common.networkError);
    } finally {
      setBusy(false);
    }
  }

  async function revoke(row: InviteRow) {
    if (!window.confirm(fmt(at.confirmRevoke, { code: row.code }))) return;
    setBusy(true);
    try {
      await api.post("/api/v1/admin/invites/revoke", { id: row.id });
      setMsg(at.revokedOk);
      await load();
    } catch (e) {
      setMsg(e instanceof ApiError ? e.message : dict.common.networkError);
    } finally {
      setBusy(false);
    }
  }

  return (
    <div className="flex flex-col gap-3">
      {msg && (
        <p className="rounded-[var(--r-md)] bg-sky-soft p-3 text-sm text-ink">
          {msg}
        </p>
      )}
      {issuedCodes && (
        <p className="num break-all rounded-[var(--r-md)] bg-mint/30 p-3 font-mono text-xs">
          {issuedCodes.join("\n")}
        </p>
      )}

      {/* 直发邀请码（0204 P1） */}
      <section className="flex flex-wrap items-end gap-2">
        <label className="flex flex-col gap-1 text-xs">
          {at.fIssueUid}
          <input
            value={issueUid}
            onChange={(e) => setIssueUid(e.target.value)}
            inputMode="numeric"
            placeholder="42"
            className={INPUT_W32}
          />
        </label>
        <label className="flex flex-col gap-1 text-xs">
          {at.fIssueCount}
          <input
            value={issueCount}
            onChange={(e) => setIssueCount(e.target.value)}
            inputMode="numeric"
            className={INPUT_CARD}
          />
        </label>
        <label className="flex flex-col gap-1 text-xs">
          {at.fIssueDays}
          <input
            value={issueDays}
            onChange={(e) => setIssueDays(e.target.value)}
            inputMode="numeric"
            placeholder={at.phIssueDays}
            className={INPUT_CARD}
          />
        </label>
        <button
          disabled={busy || !issueUid.trim()}
          onClick={() => void issue()}
          className={BTN_SM_GHOST + " border-sky text-sky-deep"}
        >
          {at.issueBtn}
        </button>
      </section>

      {/* 筛选区 */}
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
        <label className="flex flex-col gap-1 text-xs">
          {at.fEmail}
          <input
            value={email}
            onChange={(e) => {
              setEmail(e.target.value);
              setPage(1);
            }}
            placeholder={at.qAll}
            className={INPUT_W32}
          />
        </label>
        <label className="flex flex-col gap-1 text-xs">
          {at.fUsedBy}
          <input
            value={usedByName}
            onChange={(e) => {
              setUsedByName(e.target.value);
              setPage(1);
            }}
            placeholder={at.qAll}
            className={INPUT_W32}
          />
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
            <td className="colhead">{at.thEmail}</td>
            <td className="colhead">{at.thExpires}</td>
            <td className="colhead">{at.thAction}</td>
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
              <td className="text-sub">{r.email ?? "—"}</td>
              <td className="text-sub">
                {new Date(r.expires_at).toLocaleDateString(dateLocale(locale))}
              </td>
              <td>
                {r.status === 0 && (
                  <button
                    disabled={busy}
                    onClick={() => void revoke(r)}
                    className="text-tomato hover:underline"
                  >
                    {at.revokeBtn}
                  </button>
                )}
              </td>
            </tr>
          ))}
          {rows.length === 0 && (
            <tr>
              <td colSpan={8} className="py-6 text-center text-sub">
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
