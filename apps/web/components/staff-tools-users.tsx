"use client";

import { useCallback, useEffect, useState } from "react";
import { api, ApiError } from "@/lib/api-client";
import { useI18n } from "@/i18n/client";
import type { ToolTab } from "@/components/staff-tools";

/** 用户域面板（从 staff-tools.tsx 按域拆出，300 行门禁）：
 *  警告用户（warned）/ 重复 IP（ipcheck）/ 失败登录（maxlogin）/
 *  重置密码（resetpass）/ 删除被禁用户（deldisabled）/ H&R 赦免（hrpardon）。 */

interface WarnedUser { id: number; username: string; warned_until: string | null; warned_reason: string | null }
interface IpCheckRow { ip: string | null; users: number; usernames: string | null; last_seen: string | null }
interface FailedLogin { id: number; username: string | null; ip: string | null; created_at: string }

export function StaffUsersPanel({ tab, flash }: { tab: ToolTab; flash: (m: string) => void }) {
  const { dict } = useI18n();
  const t = dict.stafftools;
  const [warned, setWarned] = useState<WarnedUser[]>([]);
  const [ipRows, setIpRows] = useState<IpCheckRow[]>([]);
  const [failRows, setFailRows] = useState<FailedLogin[]>([]);
  const [busy, setBusy] = useState(false);

  const [warnUser, setWarnUser] = useState("");
  const [warnWeeks, setWarnWeeks] = useState("2");
  const [warnReason, setWarnReason] = useState("");
  const [resetId, setResetId] = useState("");
  const [resetPass, setResetPass] = useState<string | null>(null);
  const [hpUser, setHpUser] = useState("");
  const [hpTorrent, setHpTorrent] = useState("");
  const [hpNote, setHpNote] = useState("");

  const load = useCallback(async () => {
    api.get<WarnedUser[]>("/api/v1/admin/warned").then(setWarned).catch(() => setWarned([]));
    api.get<IpCheckRow[]>("/api/v1/admin/ipcheck").then(setIpRows).catch(() => setIpRows([]));
    api.get<FailedLogin[]>("/api/v1/admin/maxlogin").then(setFailRows).catch(() => setFailRows([]));
  }, []);
  useEffect(() => { load(); }, [load]);

  async function guard(fn: () => Promise<void>, ok: string) {
    setBusy(true);
    try { await fn(); flash(ok); await load(); }
    catch (e) { flash(e instanceof ApiError ? e.message : dict.common.networkError); }
    finally { setBusy(false); }
  }

  return (
    <>
      {/* 警告用户（warned） */}
      {tab === "warned" && (
        <>
          <section className="baozi-panel p-4">
            <h2 className="mb-3 text-base font-bold text-ink">{t.warnNew}</h2>
            <div className="cmgmt-form">
              <label>{t.warnUserId}<input value={warnUser} onChange={(e) => setWarnUser(e.target.value)} placeholder="4" /></label>
              <label>{t.warnWeeks}<input type="number" min={1} max={52} value={warnWeeks} onChange={(e) => setWarnWeeks(e.target.value)} /></label>
              <label>{t.fldReason}<input value={warnReason} onChange={(e) => setWarnReason(e.target.value)} /></label>
              <button className="baozi-button self-start" disabled={busy || !warnUser.trim()}
                onClick={() => guard(async () => {
                  await api.post("/api/v1/admin/warned", {
                    user_id: Number(warnUser), weeks: Number(warnWeeks), reason: warnReason || null,
                  });
                }, t.warnDone)}>
                {t.warnBtn}
              </button>
            </div>
          </section>
          <table className="nexus-table">
            <tbody>
              <tr><td className="colhead">ID</td><td className="colhead">{t.banBy === "操作人" ? "用户" : t.banBy}</td><td className="colhead">{t.warnUntil}</td><td className="colhead">{t.fldReason}</td><td className="colhead text-right">{dict.cmgmt.colActions}</td></tr>
              {warned.map((w) => (
                <tr key={w.id}>
                  <td className="num">{w.id}</td>
                  <td>{w.username}</td>
                  <td className="text-xs text-sub">{w.warned_until ? new Date(w.warned_until).toLocaleString("zh-CN") : "—"}</td>
                  <td>{w.warned_reason ?? "—"}</td>
                  <td className="text-right">
                    <button className="cmgmt-act cmgmt-act--ok"
                      onClick={() => guard(async () => { await api.del(`/api/v1/admin/warned/${w.id}`); }, t.unwarned)}>{t.unwarnBtn}</button>
                  </td>
                </tr>
              ))}
              {warned.length === 0 && <tr><td colSpan={5} className="py-6 text-center text-sub">{t.warnEmpty}</td></tr>}
            </tbody>
          </table>
        </>
      )}

      {/* 重复 IP 检测（ipcheck） */}
      {tab === "ipcheck" && (
        <table className="nexus-table">
          <tbody>
            <tr><td className="colhead">{t.fldIp}</td><td className="colhead">{t.ipAccounts}</td><td className="colhead">{t.ipUsers}</td><td className="colhead">{t.ipLastSeen}</td></tr>
            {ipRows.map((r, i) => (
              <tr key={i}>
                <td className="font-mono">{r.ip}</td>
                <td className="num">{r.users}</td>
                <td>{r.usernames ?? "—"}</td>
                <td className="text-xs text-sub">{r.last_seen ? new Date(r.last_seen).toLocaleString("zh-CN") : "—"}</td>
              </tr>
            ))}
            {ipRows.length === 0 && <tr><td colSpan={4} className="py-6 text-center text-sub">{t.ipEmpty}</td></tr>}
          </tbody>
        </table>
      )}

      {/* 失败登录（maxlogin） */}
      {tab === "maxlogin" && (
        <table className="nexus-table">
          <tbody>
            <tr><td className="colhead">{t.mlUser}</td><td className="colhead">{t.fldIp}</td><td className="colhead">{t.mailAt}</td></tr>
            {failRows.map((r) => (
              <tr key={r.id}>
                <td>{r.username ?? "（未知用户）"}</td>
                <td className="font-mono">{r.ip ?? "—"}</td>
                <td className="text-xs text-sub">{new Date(r.created_at).toLocaleString("zh-CN")}</td>
              </tr>
            ))}
            {failRows.length === 0 && <tr><td colSpan={3} className="py-6 text-center text-sub">{t.mlEmpty}</td></tr>}
          </tbody>
        </table>
      )}

      {/* 重置用户密码（reset） */}
      {tab === "resetpass" && (
        <section className="baozi-panel p-4">
          <h2 className="mb-3 text-base font-bold text-ink">{t.rpNew}</h2>
          <div className="cmgmt-form">
            <label>{t.warnUserId}<input value={resetId} onChange={(e) => setResetId(e.target.value)} placeholder="4" /></label>
            <button className="baozi-button self-start" disabled={busy || !resetId.trim()}
              onClick={() => guard(async () => {
                const r = await api.post<{ temp_password: string }>("/api/v1/admin/resetpass", { user_id: Number(resetId) });
                setResetPass(r.temp_password);
              }, t.rpDone)}>
              {t.rpBtn}
            </button>
            <p className="text-xs text-sub">{t.rpNote}</p>
          </div>
          {resetPass && (
            <p className="mt-3 rounded-[var(--r-md)] bg-sky-soft p-3 font-mono text-sm text-ink">
              {t.rpTemp}: {resetPass}
            </p>
          )}
        </section>
      )}

      {/* 删除被禁用户（deletedisabled） */}
      {tab === "deldisabled" && (
        <section className="baozi-panel p-4">
          <h2 className="mb-3 text-base font-bold text-ink">{t.ddTitle}</h2>
          <p className="mb-3 text-xs text-sub">{t.ddNote}</p>
          <button className="min-h-[40px] rounded-full bg-[var(--baozi-orange-dark)] px-5 text-sm font-bold text-white disabled:opacity-50" disabled={busy}
            onClick={() => {
              if (!window.confirm(t.ddConfirm)) return;
              void guard(async () => {
                const r = await api.post<{ deleted: number }>("/api/v1/admin/deletedisabled");
                flash(t.ddDone.replace("{n}", String(r.deleted)));
              }, "");
            }}>
            {t.ddBtn}
          </button>
        </section>
      )}

      {/* H&R 赦免（hr/pardon） */}
      {tab === "hrpardon" && (
        <section className="baozi-panel p-4">
          <h2 className="mb-3 text-base font-bold text-ink">{t.hpTitle}</h2>
          <div className="cmgmt-form">
            <label>{t.hpUser}<input value={hpUser} onChange={(e) => setHpUser(e.target.value.replace(/\D/g, ""))} placeholder="4" /></label>
            <label>{t.hpTorrent}<input value={hpTorrent} onChange={(e) => setHpTorrent(e.target.value.replace(/\D/g, ""))} placeholder="12" /></label>
            <label>{t.fldReason}<input value={hpNote} onChange={(e) => setHpNote(e.target.value)} /></label>
            <button className="baozi-button self-start" disabled={busy || !hpUser || !hpTorrent || !hpNote.trim()}
              onClick={() => guard(async () => {
                await api.post("/api/v1/admin/hr/pardon", {
                  user_id: Number(hpUser), torrent_id: Number(hpTorrent), note: hpNote,
                });
                setHpUser(""); setHpTorrent(""); setHpNote("");
              }, t.hpDone)}>
              {t.hpBtn}
            </button>
            <p className="text-xs text-sub">{t.hpNote}</p>
          </div>
        </section>
      )}
    </>
  );
}
