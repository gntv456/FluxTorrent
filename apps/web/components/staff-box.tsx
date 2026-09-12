"use client";

import { useEffect, useState } from "react";
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

/** 管理组信箱（好学站 staffbox.php 口径）：与管理组成员的往来短讯 */
export function StaffBox() {
  const { dict, locale } = useI18n();
  const t = dict.staffbox;
  const [rows, setRows] = useState<MessageRow[] | null>(null);
  const [err, setErr] = useState<string | null>(null);
  const [openId, setOpenId] = useState<number | null>(null);

  useEffect(() => {
    api
      .get<MessageRow[]>("/api/v1/messages/staff")
      .then(setRows)
      .catch((e) =>
        setErr(e instanceof ApiError ? (dict.errors[e.code] ?? e.message) : dict.common.loadFailed),
      );
  }, [dict]);

  return (
    <section className="flex flex-col gap-3">
      <div className="flex flex-wrap items-center justify-between gap-2">
        <h2 className="font-display text-xl">✉️ {t.title}</h2>
        <a
          href="/contactstaff"
          className="min-h-[44px] rounded-[10px] border border-[var(--baozi-orange-dark)] bg-[linear-gradient(135deg,var(--baozi-orange-bright),var(--baozi-orange))] px-4 text-sm font-bold leading-[44px] text-white shadow-[var(--shadow-hover)] active:scale-[0.97]"
        >
          {t.composeLink}
        </a>
      </div>

      {err && <p className="funbox__empty">{err}</p>}
      {!err && rows === null && <p className="funbox__empty">…</p>}
      {!err && rows !== null && rows.length === 0 && <p className="funbox__empty">{t.empty}</p>}

      {rows !== null && rows.length > 0 && (
        <ul className="flex flex-col gap-2">
          {rows.map((m) => (
            <li key={m.id} className="baozi-panel">
              <button
                type="button"
                className="flex w-full items-center justify-between gap-3 px-4 py-3 text-left"
                onClick={() => setOpenId(openId === m.id ? null : m.id)}
              >
                <span className="min-w-0 flex-1">
                  <b className="text-[var(--baozi-orange-dark)]">{m.counterpart ?? "Staff"}</b>
                  <span className="ml-2 text-sm font-bold">{m.subject}</span>
                </span>
                <time className="shrink-0 text-xs text-[var(--text-faint)]">
                  {new Date(m.created_at).toLocaleString(dateLocale(locale))}
                </time>
              </button>
              {openId === m.id && (
                <p className="whitespace-pre-wrap border-t border-dashed border-[var(--border-soft)] px-4 py-3 text-sm text-ink">
                  {m.body}
                </p>
              )}
            </li>
          ))}
        </ul>
      )}
    </section>
  );
}
