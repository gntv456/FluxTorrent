"use client";

import { useEffect, useState } from "react";
import { api } from "@/lib/api-client";
import { useI18n, apiErrorMessage } from "@/i18n/client";
import { useConfirm, AskDialog } from "@/components/ui/use-confirm";

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
  const s = dict.sessions;
  const [rows, setRows] = useState<SessionRow[] | null>(null);
  const [failed, setFailed] = useState(false);
  const [msg, setMsg] = useState<{ ok: boolean; text: string } | null>(null);
  const [busy, setBusy] = useState(false);
  // P2 清偿：踢出其它会话二次确认（danger：其它设备立即失效）
  const ask = useConfirm();

  useEffect(() => {
    api
      .get<SessionRow[]>("/api/v1/me/sessions")
      .then((d) => {
        setRows(d);
        setFailed(false);
      })
      .catch(() => {
        setRows([]);
        setFailed(true);
      });
  }, []);

  const revokeOthers = async () => {
    setBusy(true);
    setMsg(null);
    try {
      await api.post("/api/v1/me/sessions/revoke-others");
      setMsg({ ok: true, text: s.revokedOk });
    } catch (e) {
      // P-609：失败曾复用「未启用」并以成功色显示，像是功能没开
      setMsg({ ok: false, text: apiErrorMessage(dict, e) });
    } finally {
      setBusy(false);
    }
  };

  if (rows === null) return <span className="text-sub">…</span>;
  // 取不到会话列表不等于「没有会话」：空态会被读成账号干净
  if (failed)
    return <p className="text-xs text-warn">{dict.common.loadFailed}</p>;
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
            {s.lastAt} {new Date(r.last_at).toLocaleString()} · {s.logins}{" "}
            {r.logins}
          </span>
        </div>
      ))}
      <div className="flex items-center gap-3">
        <button
          type="button"
          onClick={() =>
            ask.open({
              title: s.revokeBtn,
              message: s.revokeConfirm,
              tone: "danger",
              // danger 默认确认文案是「删除」，踢会话不是删东西
              confirmLabel: s.revokeBtn,
              busy,
              run: revokeOthers,
            })
          }
          disabled={busy || rows.length === 0}
          className="min-h-[32px] rounded-full border border-coral px-3
            text-xs font-bold text-coral disabled:opacity-50"
        >
          {s.revokeBtn}
        </button>
        {msg && (
          <span className={msg.ok ? "text-xs text-mint" : "text-xs text-coral"}>
            {msg.text}
          </span>
        )}
      </div>
      <p className="text-[11px] text-sub">{s.hint}</p>
      <AskDialog {...ask} />
    </div>
  );
}
