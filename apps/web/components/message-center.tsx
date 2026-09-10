"use client";

import { useCallback, useEffect, useState } from "react";
import { api, ApiError } from "@/lib/api-client";
import { useI18n } from "@/i18n/client";
import { dateLocale } from "@/i18n/config";

interface MessageRow {
  id: number;
  counterpart: string | null;
  subject: string;
  body: string;
  read_at: string | null;
  created_at: string;
}

/** 站内消息中心（M17）：收件箱/已发送 + 写信（对接 /messages 三接口） */
export function MessageCenter() {
  const { dict, locale } = useI18n();
  const [box, setBox] = useState<"inbox" | "sent">("inbox");
  const [rows, setRows] = useState<MessageRow[] | null>(null);
  const [err, setErr] = useState<string | null>(null);
  const [composing, setComposing] = useState(false);
  const [to, setTo] = useState("");
  const [subject, setSubject] = useState("");
  const [body, setBody] = useState("");
  const [msg, setMsg] = useState<string | null>(null);
  const [busy, setBusy] = useState(false);
  const [openId, setOpenId] = useState<number | null>(null);

  const load = useCallback(
    (b: "inbox" | "sent") => {
      setRows(null);
      setErr(null);
      api
        .get<MessageRow[]>(`/api/v1/messages/${b}`)
        .then(setRows)
        .catch((e) =>
          setErr(
            e instanceof ApiError ? (dict.errors[e.code] ?? e.message) : dict.common.loadFailed,
          ),
        );
    },
    [dict],
  );

  useEffect(() => {
    load(box);
  }, [box, load]);

  async function send(e: React.FormEvent) {
    e.preventDefault();
    setBusy(true);
    setMsg(null);
    try {
      await api.post("/api/v1/messages", {
        to: to.trim(),
        subject: subject.trim(),
        body: body.trim(),
      });
      setMsg(dict.messages.sentOk);
      setTo("");
      setSubject("");
      setBody("");
      setComposing(false);
      setBox("sent");
    } catch (e2) {
      setMsg(
        e2 instanceof ApiError ? (dict.errors[e2.code] ?? e2.message) : dict.common.networkError,
      );
    } finally {
      setBusy(false);
    }
  }

  const inputCls =
    "min-h-[44px] w-full rounded-[var(--r-sm)] border border-[var(--baozi-line)] bg-[var(--baozi-paper)] px-3 text-sm text-ink outline-none focus:border-[var(--baozi-orange)]";

  return (
    <section className="flex flex-col gap-3">
      {/* 切换 + 写信（NexusPHP 面板头风格） */}
      <div className="flex flex-wrap items-center justify-between gap-2">
        <div className="flex gap-2">
          {(
            [
              ["inbox", dict.messages.inbox],
              ["sent", dict.messages.sent],
            ] as ["inbox" | "sent", string][]
          ).map(([k, label]) => (
            <button
              key={k}
              type="button"
              onClick={() => {
                setBox(k);
                setOpenId(null);
              }}
              aria-current={box === k ? "true" : undefined}
              className={`min-h-[44px] rounded-[10px] border px-4 text-sm font-bold ${
                box === k
                  ? "border-[var(--baozi-orange)] bg-[linear-gradient(135deg,var(--baozi-orange-bright),var(--baozi-orange))] text-white shadow-[0_5px_13px_rgba(237,90,23,0.18)]"
                  : "border-[var(--baozi-line)] bg-[rgba(255,252,245,0.84)] text-ink hover:-translate-y-px hover:text-[var(--baozi-orange)]"
              }`}
            >
              {label}
            </button>
          ))}
        </div>
        <button
          type="button"
          onClick={() => setComposing((v) => !v)}
          className="min-h-[44px] rounded-[10px] border border-[var(--baozi-orange-dark)] bg-[linear-gradient(135deg,var(--baozi-orange-bright),var(--baozi-orange))] px-4 text-sm font-bold text-white shadow-[var(--shadow-hover)] active:scale-[0.97]"
        >
          {dict.messages.compose}
        </button>
      </div>

      {composing && (
        <form
          onSubmit={send}
          className="nexus-table flex flex-col gap-2 !border-0 p-0"
        >
          <thead>
            <tr>
              <td className="colhead">{dict.messages.composeTitle}</td>
            </tr>
          </thead>
          <tbody>
            <tr>
              <td className="flex flex-col gap-2 p-3">
                <input
                  value={to}
                  onChange={(e) => setTo(e.target.value)}
                  placeholder={dict.messages.to}
                  required
                  maxLength={24}
                  className={inputCls}
                />
                <input
                  value={subject}
                  onChange={(e) => setSubject(e.target.value)}
                  placeholder={dict.messages.subject}
                  required
                  maxLength={120}
                  className={inputCls}
                />
                <textarea
                  value={body}
                  onChange={(e) => setBody(e.target.value)}
                  placeholder={dict.messages.body}
                  rows={4}
                  maxLength={5000}
                  className="min-h-[100px] rounded-[var(--r-sm)] border border-[var(--baozi-line)] bg-[var(--baozi-paper)] px-3 py-2 text-sm text-ink outline-none focus:border-[var(--baozi-orange)]"
                />
                <div className="flex items-center gap-3">
                  <button
                    type="submit"
                    disabled={busy}
                    className="min-h-[44px] rounded-[10px] border border-[var(--baozi-orange-dark)] bg-[linear-gradient(135deg,var(--baozi-orange-bright),var(--baozi-orange))] px-5 text-sm font-bold text-white active:scale-[0.97] disabled:opacity-50"
                  >
                    {busy ? dict.messages.sending : dict.messages.send}
                  </button>
                  <button
                    type="button"
                    onClick={() => setComposing(false)}
                    className="mainmenu-link min-h-[44px]"
                  >
                    {dict.messages.backToList}
                  </button>
                </div>
                {msg && (
                  <p role="status" className="text-sm text-sub">
                    {msg}
                  </p>
                )}
              </td>
            </tr>
          </tbody>
        </form>
      )}

      {err && <p className="py-4 text-center text-sm text-sub">{err}</p>}
      {!err && rows === null && (
        <p className="py-4 text-center text-sm text-sub">{dict.messages.loading}</p>
      )}
      {rows && rows.length === 0 && (
        <p className="py-4 text-center text-sm text-sub">{dict.messages.empty}</p>
      )}
      {rows && rows.length > 0 && (
        <table className="nexus-table">
          <thead>
            <tr>
              <th className="w-16">{dict.messages.readCol}</th>
              <th>{dict.messages.subject}</th>
              <th className="hidden sm:table-cell">{dict.messages.fromCol}</th>
              <th className="hidden w-44 md:table-cell">{dict.messages.timeCol}</th>
              <th className="w-16 text-right">{dict.messages.actionCol}</th>
            </tr>
          </thead>
          <tbody>
            {rows.map((m) => {
              const from = m.counterpart ?? "—";
              const time = new Date(m.created_at).toLocaleString(dateLocale(locale));
              const status = box === "inbox" ? (m.read_at ? dict.messages.readTag : dict.messages.unreadTag) : dict.messages.sentTag;
              return (
                <tr key={m.id}>
                  <td>
                    <span aria-hidden className="mr-1">
                      {box === "inbox" ? (m.read_at ? "📭" : "📬") : "📤"}
                    </span>
                    <span className={`text-[11px] ${m.read_at ? "text-sub" : "font-bold text-[var(--baozi-orange-dark)]"}`}>
                      {status}
                    </span>
                  </td>
                  <td className="min-w-0">
                    <button
                      type="button"
                      onClick={() => setOpenId(openId === m.id ? null : m.id)}
                      className={`block max-w-full truncate text-left ${m.read_at ? "text-ink" : "font-bold text-[var(--baozi-orange-dark)]"}`}
                    >
                      {m.subject}
                    </button>
                    {openId === m.id && (
                      <div className="mt-2 rounded-[var(--r-sm)] border border-dashed border-[var(--baozi-line)] bg-[var(--baozi-cream)] p-2">
                        <p className="whitespace-pre-wrap text-sm">{m.body}</p>
                        <p className="mt-1 text-[11px] text-sub md:hidden">{from} · {time}</p>
                      </div>
                    )}
                  </td>
                  <td className="hidden text-sky sm:table-cell">{from}</td>
                  <td className="hidden text-[11px] text-sub md:table-cell">{time}</td>
                  <td className="text-right">
                    <button
                      type="button"
                      onClick={() => setOpenId(openId === m.id ? null : m.id)}
                      className="text-xs font-bold text-sky hover:text-[var(--baozi-orange)]"
                    >
                      {openId === m.id ? dict.messages.backToList : dict.messages.view}
                    </button>
                  </td>
                </tr>
              );
            })}
          </tbody>
        </table>
      )}
    </section>
  );
}
