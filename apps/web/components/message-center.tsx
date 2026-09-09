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
    "min-h-[44px] rounded-[var(--r-sm)] border border-line bg-cloud px-3 text-sm outline-none focus:border-sky";

  return (
    <section className="flex flex-col gap-3 rounded-[var(--r-lg)] border border-line bg-white p-4 shadow-[var(--shadow-card)]">
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
              onClick={() => setBox(k)}
              aria-current={box === k ? "true" : undefined}
              className={`min-h-[44px] rounded-full px-4 text-sm ${
                box === k ? "bg-sky-deep text-white" : "border border-line text-ink"
              }`}
            >
              {label}
            </button>
          ))}
        </div>
        <button
          type="button"
          onClick={() => setComposing((v) => !v)}
          className="min-h-[44px] rounded-full bg-coral px-4 text-sm font-bold text-white active:scale-[0.97]"
        >
          {dict.messages.compose}
        </button>
      </div>

      {composing && (
        <form onSubmit={send} className="flex flex-col gap-2 border-b border-line pb-3">
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
            rows={3}
            maxLength={5000}
            className="min-h-[80px] rounded-[var(--r-sm)] border border-line bg-cloud px-3 py-2 text-sm outline-none focus:border-sky"
          />
          <button
            type="submit"
            disabled={busy}
            className="min-h-[44px] self-start rounded-full bg-sky px-5 text-sm font-bold text-white active:scale-[0.97] disabled:opacity-50"
          >
            {busy ? dict.messages.sending : dict.messages.send}
          </button>
        </form>
      )}
      {msg && (
        <p role="status" className="text-sm text-sub">
          {msg}
        </p>
      )}

      {err && <p className="py-4 text-center text-sm text-sub">{err}</p>}
      {!err && rows === null && (
        <p className="py-4 text-center text-sm text-sub">{dict.messages.loading}</p>
      )}
      {rows && rows.length === 0 && (
        <p className="py-4 text-center text-sm text-sub">{dict.messages.empty}</p>
      )}
      {rows && rows.length > 0 && (
        <ul className="flex flex-col divide-y divide-line">
          {rows.map((m) => (
            <li key={m.id} className="flex items-start gap-3 py-2.5">
              <span aria-hidden className="mt-0.5 text-lg">
                {m.read_at ? "📭" : "📬"}
              </span>
              <div className="min-w-0 flex-1">
                <p className={`truncate text-sm ${m.read_at ? "text-sub" : "font-bold"}`}>
                  {m.subject}
                </p>
                <p className="truncate text-xs text-sub">{m.body}</p>
                <p className="mt-0.5 text-[11px] text-sub">
                  {box === "inbox"
                    ? dict.messages.from.replace("{name}", m.counterpart ?? "—")
                    : dict.messages.toLabel.replace("{name}", m.counterpart ?? "—")}{" "}
                  · {new Date(m.created_at).toLocaleString(dateLocale(locale))}
                </p>
              </div>
            </li>
          ))}
        </ul>
      )}
    </section>
  );
}
