"use client";

import { useState } from "react";
import { api, ApiError } from "@/lib/api-client";
import { useI18n } from "@/i18n/client";

/** P2 触点 #24：自助解封（首次宽恕，0226）。
 *  被封用户先登录（登录口对被封账号放行到有限面），再到本页一键解封一次；
 *  用掉后走申诉通道。前提：站长已设 self_unban_cooldown_hours > 0。 */

export default function SelfUnbanPage() {
  const { dict } = useI18n();
  const [msg, setMsg] = useState("");
  const [busy, setBusy] = useState(false);
  const [done, setDone] = useState(false);

  async function unban() {
    setBusy(true);
    setMsg("");
    try {
      await api.post("/api/v1/auth/self-unban", {});
      setDone(true);
      setMsg(dict.selfUnban.done);
    } catch (e) {
      setMsg(e instanceof ApiError ? e.message : String(e));
    } finally {
      setBusy(false);
    }
  }

  return (
    <div className="mx-auto max-w-md px-4 py-12">
      <h1 className="text-xl font-semibold">{dict.selfUnban.title}</h1>
      <p className="mt-2 text-sm text-muted">{dict.selfUnban.note}</p>
      {msg && (
        <p className="mt-4 rounded-md border border-line bg-[var(--panel)]
          px-3 py-2 text-sm">
          {msg}
        </p>
      )}
      {!done && (
        <button
          onClick={() => void unban()}
          disabled={busy}
          className="mt-6 w-full rounded-md px-4 py-2 text-sm font-medium
            disabled:opacity-50 bg-[var(--accent)]
            text-[var(--accent-contrast)]"
        >
          {busy ? dict.selfUnban.busy : dict.selfUnban.btn}
        </button>
      )}
      <p className="mt-6 text-xs text-muted">{dict.selfUnban.closedHint}</p>
    </div>
  );
}
