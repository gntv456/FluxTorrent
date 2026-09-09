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
    <section className="rounded-[var(--r-lg)] border border-line bg-white p-4 shadow-[var(--shadow-card)]">
      <h2 className="font-display text-lg">{dict.my.passkeyTitle}</h2>
      <p className="mt-1 text-xs text-sub">{dict.my.passkeyHint}</p>
      <div className="mt-3 flex flex-wrap items-center gap-2">
        <code className="num min-h-[44px] flex items-center rounded-[var(--r-sm)] border border-line bg-cloud px-3 text-sm">
          {revealed && passkey
            ? passkey
            : revealed
              ? dict.my.passkeyHidden
              : "•••••••••••••••••••••••••••••••••"}
        </code>
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
    </section>
  );
}
