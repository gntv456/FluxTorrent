"use client";

import { useEffect, useState } from "react";
import { api } from "@/lib/api-client";
import { useI18n } from "@/i18n/client";

/**
 * 活跃会话面板（0284 P1-3）：近 24h 成功登录按 (IP, UA) 分组的近似视图 +
 * 「踢出其它会话」（撤销线 nbf=当前 token iat，其它设备立即失效）。
 */

interface SessionRow {
  ip: string;
  user_agent: string;
  last_at: string;
  logins: number;
}

export function SessionsCard() {
  const { dict } = useI18n();
  const t = dict.security2fa; // 复用安全页文案段
  const s = dict.sessions;
  const [rows, setRows] = useState<SessionRow[] | null>(null);
  const [msg, setMsg] = useState("");
  const [busy, setBusy] = useState(false);

  useEffect(() => {
    api
      .get<SessionRow[]>("/api/v1/me/sessions")
      .then(setRows)
      .catch(() => setRows([]));
  }, []);

  const revokeOthers = async () => {
    if (!window.confirm(s.revokeConfirm)) return;
    setBusy(true);
    try {
      await api.post("/api/v1/me/sessions/revoke-others");
      setMsg(s.revokedOk);
    } catch {
      setMsg(t.statusOff);
    } finally {
      setBusy(false);
    }
  };

  if (rows === null) return <span className="text-sub">…</span>;
  return (
    <div className="flex flex-col gap-2 p-3 text-sm">
      {rows.length === 0 && <span className="text-sub">{s.empty}</span>}
      {rows.map((r, i) => (
        <div
          key={i}
          className="flex flex-wrap items-baseline gap-x-3 gap-y-0.5"
        >
          <span className="font-mono text-xs">{r.ip}</span>
          <span className="max-w-[340px] truncate text-xs text-sub">
            {r.user_agent || "—"}
          </span>
          <span className="text-xs text-sub">
            {s.lastAt} {new Date(r.last_at).toLocaleString()} ·{" "}
            {s.logins} {r.logins}
          </span>
        </div>
      ))}
      <div className="flex items-center gap-3">
        <button
          type="button"
          onClick={revokeOthers}
          disabled={busy || rows.length === 0}
          className="min-h-[32px] rounded-full border border-coral px-3
            text-xs font-bold text-coral disabled:opacity-50"
        >
          {s.revokeBtn}
        </button>
        {msg && <span className="text-xs text-mint">{msg}</span>}
      </div>
      <p className="text-[11px] text-sub">{s.hint}</p>
    </div>
  );
}
