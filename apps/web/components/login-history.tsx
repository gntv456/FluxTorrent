"use client";

import { useEffect, useState } from "react";
import { api } from "@/lib/api-client";
import { useI18n } from "@/i18n/client";
import { dateLocale } from "@/i18n/config";

interface LoginEvent {
  created_at: string;
  ip: string | null;
  ok: boolean;
  user_agent: string;
  reason: number;
}

/** 登录历史（NP usercp security 口径）：最近 20 条只读表（时间/IP/结果/客户端） */
export function LoginHistory() {
  const { dict, locale } = useI18n();
  const t = dict.usercp.security;
  const [rows, setRows] = useState<LoginEvent[] | null>(null);

  useEffect(() => {
    api
      .get<LoginEvent[]>("/api/v1/me/logins")
      .then(setRows)
      .catch(() => setRows([]));
  }, []);

  if (rows === null) return <p className="text-xs text-sub">…</p>;
  if (rows.length === 0)
    return <p className="text-xs text-sub">{t.loginsEmpty}</p>;
  const reasonLabel = (r: number, ok: boolean) => {
    if (ok) return t.loginOk;
    return (
      (
        {
          1: t.loginBadPw,
          2: t.loginBad2fa,
          3: t.loginDormant,
          4: t.loginUnknown,
        } as Record<number, string>
      )[r] ?? t.loginFailed
    );
  };
  return (
    <div className="baozi-wide-table-scroll">
      <table className="nexus-table">
        <tbody>
          <tr>
            <td className="colhead">{t.loginAt}</td>
            <td className="colhead">IP</td>
            <td className="colhead">{t.loginResult}</td>
            <td className="colhead">{t.loginClient}</td>
          </tr>
          {rows.map((r, i) => (
            <tr key={i}>
              <td className="num text-xs">
                {new Date(r.created_at).toLocaleString(dateLocale(locale))}
              </td>
              <td className="num text-xs">{r.ip ?? "—"}</td>
              <td
                className={`text-xs ${r.ok ? "text-emerald-600" : "text-coral"}`}
              >
                {reasonLabel(r.reason, r.ok)}
              </td>
              <td
                className="max-w-[280px] truncate text-xs text-sub"
                title={r.user_agent}
              >
                {r.user_agent || "—"}
              </td>
            </tr>
          ))}
        </tbody>
      </table>
    </div>
  );
}
