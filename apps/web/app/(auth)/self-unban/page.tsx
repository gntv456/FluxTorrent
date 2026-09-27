"use client";

import { useState } from "react";
import { api, ApiError } from "@/lib/api-client";

/** P2 触点 #24：自助解封（首次宽恕，0226）。
 *  被封用户先登录（登录口对被封账号放行到有限面），再到本页一键解封一次；
 *  用掉后走申诉通道。前提：站长已设 self_unban_cooldown_hours > 0。 */

export default function SelfUnbanPage() {
  const [msg, setMsg] = useState("");
  const [busy, setBusy] = useState(false);
  const [done, setDone] = useState(false);

  async function unban() {
    setBusy(true);
    setMsg("");
    try {
      await api.post("/api/v1/auth/self-unban", {});
      setDone(true);
      setMsg("已解封。请刷新后进入站点；再次违规将只能走申诉通道。");
    } catch (e) {
      setMsg(e instanceof ApiError ? e.message : String(e));
    } finally {
      setBusy(false);
    }
  }

  return (
    <div className="mx-auto max-w-md px-4 py-12">
      <h1 className="text-xl font-semibold">自助解封（首次宽恕）</h1>
      <p className="mt-2 text-sm text-muted">
        每个账号有一次自助解封机会：适用于首次误封/轻微违规。
        使用后再次被封只能通过申诉通道联系管理组。
      </p>
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
          {busy ? "处理中…" : "立即解封我的账号"}
        </button>
      )}
      <p className="mt-6 text-xs text-muted">
        若提示未开放自助解封，说明本站未启用该功能，请通过「联系我们」申诉。
      </p>
    </div>
  );
}
