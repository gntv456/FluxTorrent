"use client";

/**
 * 安全域·邮箱黑白名单 + IP 测试子面板（从
 * components/staff-tools-security.tsx 按域拆出）：
 * emailbans 规则 CRUD 与 testip 封禁态查询。动作用父级 guard 上抛。
 */

import { useState } from "react";
import { api, ApiError } from "@/lib/api-client";
import { useI18n } from "@/i18n/client";
import type { TestIpResult } from "./staff-tools-security-shared";

interface SecPanelsProps {
  tab: "emailbans" | "testip";
  emailBans: {
    id: number;
    pattern: string;
    mode: string;
    note: string | null;
    created_by: string | null;
    created_at: string;
  }[];
  busy: boolean;
  guard: (fn: () => Promise<void>, ok: string) => Promise<void>;
  flash: (m: string) => void;
}

export function StaffSecPanels({
  tab,
  emailBans,
  busy,
  guard,
  flash,
}: SecPanelsProps) {
  const { dict } = useI18n();
  const t = dict.stafftools;
  const [ebPattern, setEbPattern] = useState("");
  const [ebMode, setEbMode] = useState("ban");
  const [ebNote, setEbNote] = useState("");
  const [testIpQuery, setTestIpQuery] = useState("");
  const [testIpResult, setTestIpResult] = useState<TestIpResult | null>(null);
  const [busyLocal, setBusyLocal] = useState(false);
  const busy2 = busy || busyLocal;

  return (
    <>
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
                disabled={busy2 || !ebPattern.trim()}
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
              disabled={busy2 || !testIpQuery.trim()}
              onClick={() =>
                void (async () => {
                  setBusyLocal(true);
                  try {
                    const r = await api.get<TestIpResult>(
                      `/api/v1/admin/testip?ip=${encodeURIComponent(
                        testIpQuery.trim(),
                      )}`,
                    );
                    setTestIpResult(r);
                  } catch (e) {
                    flash(
                      e instanceof ApiError
                        ? e.message
                        : dict.common.networkError,
                    );
                  } finally {
                    setBusyLocal(false);
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
