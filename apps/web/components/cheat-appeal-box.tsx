"use client";

import { useEffect, useState } from "react";
import { api, ApiError } from "@/lib/api-client";
import { apiErrorMessage, useI18n } from "@/i18n/client";

/** 我的作弊申诉行（GET /my/cheat-appeals，0369 M3） */
interface MyAppeal {
  id: number;
  case_id: number | null;
  body: string;
  status: string;
  outcome: string | null;
  result_note: string | null;
  created_at: string;
}

/**
 * 作弊处置申诉（0369 M3，用户侧）：
 *   ① 一次性令牌申诉——停权/封禁时随站内信发出，免登录，用后即焚。
 *   ② 我的作弊申诉——列出结论与案件处置状态。
 * 与通用「申诉通道」(/appeals) 同页并列，但走独立的案件强关联链路。
 */
export function CheatAppealBox() {
  const { dict } = useI18n();
  const t = dict.cheaterbox;
  const [rows, setRows] = useState<MyAppeal[] | null>(null);
  // P-614：取数失败曾被演成「暂无申诉记录」——会员会以为自己的申诉没了
  const [failed, setFailed] = useState(false);
  const [loggedIn, setLoggedIn] = useState(true);
  const [token, setToken] = useState("");
  const [body, setBody] = useState("");
  const [busy, setBusy] = useState(false);
  const [msg, setMsg] = useState<string | null>(null);
  const [err, setErr] = useState<string | null>(null);

  useEffect(() => {
    api
      .get<MyAppeal[]>("/api/v1/my/cheat-appeals")
      .then(setRows)
      .catch((e) => {
        if (e instanceof ApiError && e.code === 2001) {
          setLoggedIn(false);
          return;
        }
        setRows([]);
        setFailed(true);
      });
  }, []);

  async function submit(e: React.FormEvent) {
    e.preventDefault();
    setBusy(true);
    setErr(null);
    setMsg(null);
    try {
      await api.post("/api/v1/appeals/token", {
        token: token.trim(),
        body: body.trim(),
      });
      setMsg(t.tokenOk);
      setToken("");
      setBody("");
    } catch (e2) {
      setErr(
        e2 instanceof ApiError
          ? apiErrorMessage(dict, e2)
          : dict.common.networkError,
      );
    } finally {
      setBusy(false);
    }
  }

  const inputCls =
    "min-h-[44px] rounded-[var(--r-sm)] border border-line bg-cloud px-3 " +
    "text-sm outline-none focus:border-sky";
  const outcomeLabel = (o: string) =>
    o === "overturned"
      ? t.outcomeOverturned
      : o === "mitigated"
        ? t.outcomeMitigated
        : t.outcomeUpheld;

  return (
    <section
      className={
        "flex flex-col gap-3 rounded-[var(--r-lg)] border border-line " +
        "bg-[var(--surface-card)] p-4 shadow-[var(--shadow-card)]"
      }
    >
      <h2 className="font-display text-lg">{t.tokenTitle}</h2>
      <p className="-mt-2 text-xs text-sub">{t.tokenHint}</p>
      <form onSubmit={submit} className="flex flex-col gap-3">
        <label className="flex flex-col gap-1">
          <span className="text-sm text-sub">{t.tokenLabel}</span>
          <input
            value={token}
            onChange={(e) => setToken(e.target.value)}
            required
            className={`${inputCls} font-mono`}
            placeholder={t.tokenPlaceholder}
          />
        </label>
        <textarea
          value={body}
          onChange={(e) => setBody(e.target.value)}
          required
          minLength={10}
          maxLength={4000}
          rows={4}
          placeholder={t.tokenBodyPlaceholder}
          className="min-h-[88px] rounded-[var(--r-sm)] border border-line
            bg-cloud px-3 py-2 text-sm outline-none focus:border-sky"
        />
        {err && (
          <p role="alert" className="text-sm text-danger">
            {err}
          </p>
        )}
        {msg && (
          <p role="status" className="text-sm text-mint">
            {msg}
          </p>
        )}
        <button
          type="submit"
          disabled={busy}
          className="min-h-[44px] rounded-[var(--r-sm)] bg-sky px-4 text-sm
            font-bold text-white disabled:opacity-50"
        >
          {t.tokenSubmit}
        </button>
      </form>

      {loggedIn ? (
        <div className="flex flex-col gap-2 border-t border-line pt-3">
          <h3 className="font-display text-base">{t.myAppealTitle}</h3>
          {rows === null && <p className="text-sm text-sub">…</p>}
          {failed && (
            <p role="alert" className="text-sm text-sub">
              {dict.common.loadFailed}
            </p>
          )}
          {!failed && rows?.length === 0 && (
            <p className="text-sm text-sub">{t.myAppealEmpty}</p>
          )}
          {!failed && rows && rows.length > 0 && (
            <ul className="flex flex-col divide-y divide-line">
              {rows.map((r) => (
                <li key={r.id} className="py-2">
                  <div className="flex items-center justify-between gap-2">
                    <span className="text-sm font-bold">
                      {r.case_id ? `${t.colAppealCase} #${r.case_id}` : "—"}
                    </span>
                    <span className="text-xs text-sub">
                      {r.status === "open"
                        ? t.appealStatusOpen
                        : r.outcome
                          ? outcomeLabel(r.outcome)
                          : t.appealStatusDone}
                    </span>
                  </div>
                  <p className="mt-1 text-sm whitespace-pre-wrap">{r.body}</p>
                  {r.result_note && (
                    <p className="mt-1 text-sm text-sub">
                      {r.result_note}
                    </p>
                  )}
                  <p className="mt-1 text-[11px] text-sub">
                    {new Date(r.created_at).toLocaleString()}
                  </p>
                </li>
              ))}
            </ul>
          )}
        </div>
      ) : null}
    </section>
  );
}
