"use client";

import { useCallback, useEffect, useState } from "react";
import { api, ApiError } from "@/lib/api-client";
import { useI18n } from "@/i18n/client";
import type { ToolTab } from "@/components/staff-tools";

/** 安全域面板（从 staff-tools.tsx 按域拆出，300 行门禁）：
 *  封禁系统（bans）/ 批量邮件（mail）/ 邮箱黑白名单（emailbans）/ IP 测试（testip）。 */

interface BanItem {
  id: number;
  ip: string;
  reason: string | null;
  banned_by: string | null;
  created_at: string;
}
interface MailItem {
  id: number;
  subject: string;
  recipients: number;
  created_at: string;
  sender: string | null;
}
interface EmailBan {
  id: number;
  pattern: string;
  mode: string;
  note: string | null;
  created_by: string | null;
  created_at: string;
}
interface TestIpResult {
  ip: string;
  banned: boolean;
  reason: string | null;
  by: string | null;
  seen_users: string[];
}

export function StaffSecurityPanel({
  tab,
  flash,
}: {
  tab: ToolTab;
  flash: (m: string) => void;
}) {
  const { dict } = useI18n();
  const t = dict.stafftools;
  const [bans, setBans] = useState<BanItem[]>([]);
  const [mails, setMails] = useState<MailItem[]>([]);
  const [emailBans, setEmailBans] = useState<EmailBan[]>([]);
  const [testIpResult, setTestIpResult] = useState<TestIpResult | null>(null);
  const [busy, setBusy] = useState(false);

  const [banIp, setBanIp] = useState("");
  const [banReason, setBanReason] = useState("");
  const [mailSubject, setMailSubject] = useState("");
  const [mailBody, setMailBody] = useState("");
  const [ebPattern, setEbPattern] = useState("");
  const [ebMode, setEbMode] = useState("ban");
  const [ebNote, setEbNote] = useState("");
  const [testIpQuery, setTestIpQuery] = useState("");

  const load = useCallback(async () => {
    api
      .get<BanItem[]>("/api/v1/admin/bans")
      .then(setBans)
      .catch(() => setBans([]));
    api
      .get<MailItem[]>("/api/v1/admin/massmail")
      .then(setMails)
      .catch(() => setMails([]));
    api
      .get<EmailBan[]>("/api/v1/admin/emailbans")
      .then(setEmailBans)
      .catch(() => setEmailBans([]));
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
      {/* 封禁系统 */}
      {tab === "bans" && (
        <>
          <section className="baozi-panel p-4">
            <h2 className="mb-3 text-base font-bold text-ink">{t.banNew}</h2>
            <div className="cmgmt-form">
              <label>
                {t.fldIp}
                <input
                  value={banIp}
                  onChange={(e) => setBanIp(e.target.value)}
                  placeholder="203.0.113.10"
                />
              </label>
              <label>
                {t.fldReason}
                <input
                  value={banReason}
                  onChange={(e) => setBanReason(e.target.value)}
                />
              </label>
              <button
                className="baozi-button self-start"
                disabled={busy || !banIp.trim()}
                onClick={() =>
                  guard(async () => {
                    await api.post("/api/v1/admin/bans", {
                      ip: banIp,
                      reason: banReason,
                    });
                    setBanIp("");
                    setBanReason("");
                  }, t.banAdded)
                }
              >
                {t.btnBan}
              </button>
            </div>
          </section>
          <table className="nexus-table">
            <tbody>
              <tr>
                <td className="colhead">{t.fldIp}</td>
                <td className="colhead">{t.fldReason}</td>
                <td className="colhead">{t.banBy}</td>
                <td className="colhead text-right">{dict.cmgmt.colActions}</td>
              </tr>
              {bans.map((b) => (
                <tr key={b.id}>
                  <td className="font-mono">{b.ip}</td>
                  <td>{b.reason ?? "—"}</td>
                  <td>{b.banned_by ?? "—"}</td>
                  <td className="text-right">
                    <button
                      className="cmgmt-act cmgmt-act--ok"
                      onClick={() =>
                        guard(async () => {
                          await api.del(`/api/v1/admin/bans/${b.id}`);
                        }, t.unbanned)
                      }
                    >
                      {t.btnUnban}
                    </button>
                  </td>
                </tr>
              ))}
              {bans.length === 0 && (
                <tr>
                  <td colSpan={4} className="py-6 text-center text-sub">
                    {t.banEmpty}
                  </td>
                </tr>
              )}
            </tbody>
          </table>
        </>
      )}

      {/* 批量邮件 */}
      {tab === "mail" && (
        <>
          <section className="baozi-panel p-4">
            <h2 className="mb-3 text-base font-bold text-ink">{t.mailNew}</h2>
            <div className="cmgmt-form">
              <label>
                {t.fldSubject}
                <input
                  value={mailSubject}
                  onChange={(e) => setMailSubject(e.target.value)}
                />
              </label>
              <label>
                {t.fldBody}
                <textarea
                  rows={6}
                  value={mailBody}
                  onChange={(e) => setMailBody(e.target.value)}
                />
              </label>
              <button
                className="baozi-button self-start"
                disabled={busy || !mailSubject.trim() || !mailBody.trim()}
                onClick={() =>
                  guard(async () => {
                    await api.post("/api/v1/admin/massmail", {
                      subject: mailSubject,
                      body: mailBody,
                    });
                    setMailSubject("");
                    setMailBody("");
                  }, t.mailQueued)
                }
              >
                {t.btnSend}
              </button>
            </div>
          </section>
          <table className="nexus-table">
            <tbody>
              <tr>
                <td className="colhead">{t.fldSubject}</td>
                <td className="colhead">{t.mailRecipients}</td>
                <td className="colhead">{t.mailSender}</td>
                <td className="colhead">{t.mailAt}</td>
              </tr>
              {mails.map((m) => (
                <tr key={m.id}>
                  <td>{m.subject}</td>
                  <td className="num">{m.recipients}</td>
                  <td>{m.sender ?? "—"}</td>
                  <td className="text-xs text-sub">
                    {new Date(m.created_at).toLocaleString("zh-CN")}
                  </td>
                </tr>
              ))}
              {mails.length === 0 && (
                <tr>
                  <td colSpan={4} className="py-6 text-center text-sub">
                    {t.mailEmpty}
                  </td>
                </tr>
              )}
            </tbody>
          </table>
        </>
      )}

      {/* 邮箱黑白名单（bannedemails/allowedemails） */}
      {tab === "emailbans" && (
        <>
          <section className="baozi-panel p-4">
            <h2 className="mb-3 text-base font-bold text-ink">{t.ebNew}</h2>
            <div className="cmgmt-form">
              <label>
                {t.ebPattern}
                <input
                  value={ebPattern}
                  onChange={(e) => setEbPattern(e.target.value)}
                  placeholder="@spam.example / user@ / a@b.com"
                />
              </label>
              <label>
                {t.ebMode}
                <select
                  value={ebMode}
                  onChange={(e) => setEbMode(e.target.value)}
                >
                  <option value="ban">{t.ebModeBan}</option>
                  <option value="allow">{t.ebModeAllow}</option>
                </select>
              </label>
              <label>
                {t.ebNote}
                <input
                  value={ebNote}
                  onChange={(e) => setEbNote(e.target.value)}
                />
              </label>
              <button
                className="baozi-button self-start"
                disabled={busy || !ebPattern.trim()}
                onClick={() =>
                  guard(async () => {
                    await api.post("/api/v1/admin/emailbans", {
                      pattern: ebPattern,
                      mode: ebMode,
                      note: ebNote || null,
                    });
                    setEbPattern("");
                    setEbNote("");
                  }, t.saved)
                }
              >
                {t.btnSave}
              </button>
            </div>
          </section>
          <table className="nexus-table">
            <tbody>
              <tr>
                <td className="colhead">{t.ebPattern}</td>
                <td className="colhead">{t.ebMode}</td>
                <td className="colhead">{t.ebNote}</td>
                <td className="colhead text-right">{dict.cmgmt.colActions}</td>
              </tr>
              {emailBans.map((e) => (
                <tr key={e.id}>
                  <td className="font-mono">{e.pattern}</td>
                  <td>{e.mode === "ban" ? t.ebModeBan : t.ebModeAllow}</td>
                  <td>{e.note ?? "—"}</td>
                  <td className="text-right">
                    <button
                      className="cmgmt-act cmgmt-act--danger"
                      onClick={() =>
                        guard(async () => {
                          await api.del(`/api/v1/admin/emailbans/${e.id}`);
                        }, t.deleted)
                      }
                    >
                      {dict.cmgmt.btnDelete}
                    </button>
                  </td>
                </tr>
              ))}
              {emailBans.length === 0 && (
                <tr>
                  <td colSpan={4} className="py-6 text-center text-sub">
                    {t.ebEmpty}
                  </td>
                </tr>
              )}
            </tbody>
          </table>
        </>
      )}

      {/* IP 测试（testip） */}
      {tab === "testip" && (
        <section className="baozi-panel p-4">
          <h2 className="mb-3 text-base font-bold text-ink">{t.tiTitle}</h2>
          <div className="cmgmt-form">
            <label>
              {t.fldIp}
              <input
                value={testIpQuery}
                onChange={(e) => setTestIpQuery(e.target.value)}
                placeholder="203.0.113.10"
              />
            </label>
            <button
              className="baozi-button self-start"
              disabled={busy || !testIpQuery.trim()}
              onClick={() =>
                void (async () => {
                  setBusy(true);
                  try {
                    const r = await api.get<TestIpResult>(
                      `/api/v1/admin/testip?ip=${encodeURIComponent(testIpQuery.trim())}`,
                    );
                    setTestIpResult(r);
                  } catch (e) {
                    flash(
                      e instanceof ApiError
                        ? e.message
                        : dict.common.networkError,
                    );
                  } finally {
                    setBusy(false);
                  }
                })()
              }
            >
              {t.tiBtn}
            </button>
          </div>
          {testIpResult && (
            <table className="nexus-table mt-3">
              <tbody>
                <tr>
                  <td className="rowhead">{t.fldIp}</td>
                  <td className="rowfollow font-mono">{testIpResult.ip}</td>
                </tr>
                <tr>
                  <td className="rowhead">{t.tiBanned}</td>
                  <td className="rowfollow">
                    {testIpResult.banned ? `⛔ ${t.tiYes}` : `✅ ${t.tiNo}`}
                  </td>
                </tr>
                {testIpResult.banned && (
                  <>
                    <tr>
                      <td className="rowhead">{t.fldReason}</td>
                      <td className="rowfollow">
                        {testIpResult.reason ?? "—"}
                      </td>
                    </tr>
                    <tr>
                      <td className="rowhead">{t.banBy}</td>
                      <td className="rowfollow">{testIpResult.by ?? "—"}</td>
                    </tr>
                  </>
                )}
                <tr>
                  <td className="rowhead">{t.tiSeenUsers}</td>
                  <td className="rowfollow">
                    {testIpResult.seen_users.length > 0
                      ? testIpResult.seen_users.join(", ")
                      : "—"}
                  </td>
                </tr>
              </tbody>
            </table>
          )}
        </section>
      )}
    </>
  );
}
