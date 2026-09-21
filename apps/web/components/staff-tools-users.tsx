"use client";

import { useCallback, useEffect, useState } from "react";
import { api, ApiError } from "@/lib/api-client";
import { useI18n } from "@/i18n/client";
import type { ToolTab } from "@/components/staff-tools";
import {
  DelDisabledSection,
  HrPardonSection,
  ResetPassSection,
} from "./staff-tools-users-tools";

/** 用户域面板（从 staff-tools.tsx 按域拆出，300 行门禁）：
 *  警告用户（warned）/ 重复 IP（ipcheck）/ 失败登录（maxlogin）/
 *  重置密码（resetpass）/ 删除被禁用户（deldisabled）/ H&R 赦免（hrpardon）。
 *  三个工具子面板拆至 ./staff-tools-users-tools.tsx。 */

interface WarnedUser {
  id: number;
  username: string;
  warned_until: string | null;
  warned_reason: string | null;
}
interface IpCheckRow {
  ip: string | null;
  users: number;
  usernames: string | null;
  last_seen: string | null;
}
interface FailedLogin {
  id: number;
  username: string | null;
  ip: string | null;
  created_at: string;
}

export function StaffUsersPanel({
  tab,
  flash,
}: {
  tab: ToolTab;
  flash: (m: string) => void;
}) {
  const { dict } = useI18n();
  const t = dict.stafftools;
  const [warned, setWarned] = useState<WarnedUser[]>([]);
  const [ipRows, setIpRows] = useState<IpCheckRow[]>([]);
  const [failRows, setFailRows] = useState<FailedLogin[]>([]);
  const [busy, setBusy] = useState(false);

  const [warnUser, setWarnUser] = useState("");
  const [warnWeeks, setWarnWeeks] = useState("2");
  const [warnReason, setWarnReason] = useState("");

  const load = useCallback(async () => {
    api
      .get<WarnedUser[]>("/api/v1/admin/warned")
      .then(setWarned)
      .catch(() => setWarned([]));
    api
      .get<IpCheckRow[]>("/api/v1/admin/ipcheck")
      .then(setIpRows)
      .catch(() => setIpRows([]));
    api
      .get<FailedLogin[]>("/api/v1/admin/maxlogin")
      .then(setFailRows)
      .catch(() => setFailRows([]));
  }, []);
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
    <>
      {/* 警告用户（warned） */}
      {tab === "warned" && (
        <>
          <section className="baozi-panel p-4">
            <h2 className="mb-3 text-base font-bold text-ink">{t.warnNew}</h2>
            <div className="cmgmt-form">
              <label>
                {t.warnUserId}
                <input
                  value={warnUser}
                  onChange={(e) => setWarnUser(e.target.value)}
                  placeholder="4"
                />
              </label>
              <label>
                {t.warnWeeks}
                <input
                  type="number"
                  min={1}
                  max={52}
                  value={warnWeeks}
                  onChange={(e) => setWarnWeeks(e.target.value)}
                />
              </label>
              <label>
                {t.fldReason}
                <input
                  value={warnReason}
                  onChange={(e) => setWarnReason(e.target.value)}
                />
              </label>
              <button
                className="baozi-button self-start"
                disabled={busy || !warnUser.trim()}
                onClick={() =>
                  guard(async () => {
                    await api.post("/api/v1/admin/warned", {
                      user_id: Number(warnUser),
                      weeks: Number(warnWeeks),
                      reason: warnReason || null,
                    });
                  }, t.warnDone)
                }
              >
                {t.warnBtn}
              </button>
            </div>
          </section>
          <table className="nexus-table">
            <tbody>
              <tr>
                <td className="colhead">ID</td>
                <td className="colhead">
                  {t.banBy === "操作人" ? "用户" : t.banBy}
                </td>
                <td className="colhead">{t.warnUntil}</td>
                <td className="colhead">{t.fldReason}</td>
                <td className="colhead text-right">{dict.cmgmt.colActions}</td>
              </tr>
              {warned.map((w) => (
                <tr key={w.id}>
                  <td className="num">{w.id}</td>
                  <td>{w.username}</td>
                  <td className="text-xs text-sub">
                    {w.warned_until
                      ? new Date(w.warned_until).toLocaleString("zh-CN")
                      : "—"}
                  </td>
                  <td>{w.warned_reason ?? "—"}</td>
                  <td className="text-right">
                    <button
                      className="cmgmt-act cmgmt-act--ok"
                      onClick={() =>
                        guard(async () => {
                          await api.del(`/api/v1/admin/warned/${w.id}`);
                        }, t.unwarned)
                      }
                    >
                      {t.unwarnBtn}
                    </button>
                  </td>
                </tr>
              ))}
              {warned.length === 0 && (
                <tr>
                  <td colSpan={5} className="py-6 text-center text-sub">
                    {t.warnEmpty}
                  </td>
                </tr>
              )}
            </tbody>
          </table>
        </>
      )}

      {/* 重复 IP 检测（ipcheck） */}
      {tab === "ipcheck" && (
        <table className="nexus-table">
          <tbody>
            <tr>
              <td className="colhead">{t.fldIp}</td>
              <td className="colhead">{t.ipAccounts}</td>
              <td className="colhead">{t.ipUsers}</td>
              <td className="colhead">{t.ipLastSeen}</td>
            </tr>
            {ipRows.map((r, i) => (
              <tr key={i}>
                <td className="font-mono">{r.ip}</td>
                <td className="num">{r.users}</td>
                <td>{r.usernames ?? "—"}</td>
                <td className="text-xs text-sub">
                  {r.last_seen
                    ? new Date(r.last_seen).toLocaleString("zh-CN")
                    : "—"}
                </td>
              </tr>
            ))}
            {ipRows.length === 0 && (
              <tr>
                <td colSpan={4} className="py-6 text-center text-sub">
                  {t.ipEmpty}
                </td>
              </tr>
            )}
          </tbody>
        </table>
      )}

      {/* 失败登录（maxlogin） */}
      {tab === "maxlogin" && (
        <table className="nexus-table">
          <tbody>
            <tr>
              <td className="colhead">{t.mlUser}</td>
              <td className="colhead">{t.fldIp}</td>
              <td className="colhead">{t.mailAt}</td>
            </tr>
            {failRows.map((r) => (
              <tr key={r.id}>
                <td>{r.username ?? "（未知用户）"}</td>
                <td className="font-mono">{r.ip ?? "—"}</td>
                <td className="text-xs text-sub">
                  {new Date(r.created_at).toLocaleString("zh-CN")}
                </td>
              </tr>
            ))}
            {failRows.length === 0 && (
              <tr>
                <td colSpan={3} className="py-6 text-center text-sub">
                  {t.mlEmpty}
                </td>
              </tr>
            )}
          </tbody>
        </table>
      )}

      {/* 重置用户密码（reset） */}
      {tab === "resetpass" && <ResetPassSection busy={busy} guard={guard} />}

      {/* 删除被禁用户（deletedisabled） */}
      {tab === "deldisabled" && (
        <DelDisabledSection busy={busy} guard={guard} flash={flash} />
      )}

      {/* H&R 赦免（hr/pardon） */}
      {tab === "hrpardon" && <HrPardonSection busy={busy} guard={guard} />}
    </>
  );
}
