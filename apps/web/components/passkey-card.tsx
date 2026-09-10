"use client";

import { useState } from "react";
import { api, ApiError } from "@/lib/api-client";
import { useI18n } from "@/i18n/client";

/** Passkey 管理（M09 安全设置）：展示脱敏 passkey + 轮换 */
export function PasskeyCard() {
  const { dict } = useI18n();
  const [revealed, setRevealed] = useState(false);
  const [passkey, setPasskey] = useState<string | null>(null);
  const [msg, setMsg] = useState<string | null>(null);
  const [busy, setBusy] = useState(false);

  async function load() {
    try {
      // 后端未提供明文查询（只存哈希口径不一致时回退），首次点击时尝试展示占位
      setRevealed((v) => !v);
    } catch {
      /* noop */
    }
  }

  async function rotate() {
    if (!window.confirm(dict.my.passkeyConfirm)) return;
    setBusy(true);
    setMsg(null);
    try {
      const r = await api.post<{ passkey: string }>("/api/v1/me/passkey/rotate", {});
      setPasskey(r.passkey);
      setRevealed(true);
      setMsg(dict.my.passkeyRotated);
    } catch (e) {
      setMsg(
        e instanceof ApiError ? (dict.errors[e.code] ?? e.message) : dict.common.networkError,
      );
    } finally {
      setBusy(false);
    }
  }

  return (
    <section className="nexus-detail">
      <table className="nexus-table nexus-form">
        <thead>
          <tr>
            <td colSpan={2} className="colhead">
              {dict.my.passkeyTitle}
            </td>
          </tr>
        </thead>
        <tbody>
          <tr>
            <td className="rowhead">{dict.my.passkeyTitle}</td>
            <td className="rowfollow">
              <code className="num break-all">
                {revealed && passkey
                  ? passkey
                  : revealed
                    ? dict.my.passkeyHidden
                    : "••••••••••••••••••••••••••••••••"}
              </code>
            </td>
          </tr>
          <tr>
            <td className="rowhead">{dict.my.passkeyHint}</td>
            <td className="rowfollow">
              <div className="flex flex-wrap items-center gap-2">
                <button
                  type="button"
                  onClick={load}
                  className="min-h-[44px] rounded-full border border-line px-4 text-sm text-sub active:scale-[0.97]"
                >
                  {revealed ? dict.my.passkeyHide : dict.my.passkeyShow}
                </button>
                <button
                  type="button"
                  onClick={rotate}
                  disabled={busy}
                  className="min-h-[44px] rounded-full bg-coral px-5 text-sm font-bold text-white active:scale-[0.97] disabled:opacity-50"
                >
                  {busy ? dict.my.passkeyRotating : dict.my.passkeyRotate}
                </button>
              </div>
              {msg && (
                <p role="status" className="mt-2 text-sm text-sub">
                  {msg}
                </p>
              )}
            </td>
          </tr>
        </tbody>
      </table>
    </section>
  );
}
