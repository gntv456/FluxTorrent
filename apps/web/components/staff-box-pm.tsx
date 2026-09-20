"use client";

import { useI18n } from "@/i18n/client";
import { dateLocale } from "@/i18n/config";
import type { MessageRow } from "@/components/staff-box-desk";

/** 管理组私信视图（从 components/staff-box.tsx 按域拆出）：
 *  往来短讯手风琴列表（展开看原文 + 回复按钮）与回复表单。
 *  数据与动作由 StaffBox 注入。 */

export function PmList({
  rows,
  openId,
  onToggleOpen,
  onReply,
}: {
  rows: MessageRow[];
  openId: number | null;
  onToggleOpen: (id: number) => void;
  onReply: (m: MessageRow) => void;
}) {
  const { dict, locale } = useI18n();
  return (
    <ul className="flex flex-col gap-2">
      {rows.map((m) => (
        <li key={m.id} className="baozi-panel">
          <button
            type="button"
            className="flex w-full items-center justify-between gap-3 px-4 py-3 text-left"
            onClick={() => onToggleOpen(m.id)}
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
            <div className="border-t border-dashed border-[var(--border-soft)] px-4 py-3">
              <p className="whitespace-pre-wrap text-sm text-ink">{m.body}</p>
              <div className="mt-2 flex justify-end">
                <button
                  type="button"
                  onClick={() => onReply(m)}
                  className="min-h-[32px] rounded-full border border-[var(--baozi-orange-dark)] px-3 text-xs font-bold text-[var(--baozi-orange-dark)] hover:bg-[var(--baozi-orange)] hover:text-white"
                >
                  ↩ {dict.messages.reply}
                </button>
              </div>
            </div>
          )}
        </li>
      ))}
    </ul>
  );
}

export function PmReplyForm({
  replyTo,
  replyBody,
  setReplyBody,
  busy,
  onSubmit,
  onClose,
}: {
  replyTo: MessageRow;
  replyBody: string;
  setReplyBody: (v: string) => void;
  busy: boolean;
  onSubmit: (e: React.FormEvent) => void;
  onClose: () => void;
}) {
  const { dict } = useI18n();
  return (
    <section className="baozi-panel">
      <header className="baozi-panel__head">
        <h2>
          ↩ {dict.messages.reply} · {replyTo.counterpart ?? "Staff"} — {replyTo.subject}
        </h2>
        <button type="button" className="min-h-[32px] rounded-full border border-line px-3 text-xs font-bold" onClick={onClose}>
          ✕
        </button>
      </header>
      <form
        className="flex flex-col gap-3 px-4 pb-4"
        onSubmit={(e) => {
          e.preventDefault();
          onSubmit(e);
        }}
      >
        <textarea
          rows={4}
          value={replyBody}
          onChange={(e) => setReplyBody(e.target.value)}
          className="min-h-[44px] w-full rounded-[var(--r-sm)] border border-[var(--baozi-line)] bg-[var(--baozi-paper)] px-3 py-2 text-sm text-ink outline-none focus:border-[var(--baozi-orange)]"
        />
        <button type="submit" className="baozi-button self-start disabled:opacity-50" disabled={busy || !replyBody.trim()}>
          {dict.messages.send}
        </button>
      </form>
    </section>
  );
}
