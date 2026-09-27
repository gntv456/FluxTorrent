"use client";

import { api, ApiError } from "@/lib/api-client";
import { useI18n } from "@/i18n/client";
import { dateLocale } from "@/i18n/config";
import { MarkdownRenderer } from "@/components/forum-markdown";

/** 消息中心共享契约（从 components/message-center.tsx 按域拆出） */

export interface MessageRow {
  id: number;
  counterpart: string | null;
  subject: string;
  body: string;
  read_at: string | null;
  created_at: string;
  unread?: boolean | null;
  folder?: number | null;
  /** 系统通知（sender_id IS NULL，0123 视觉区分用）：🔔 徽标 + 禁用回复/转发 */
  is_system?: boolean | null;
}

export interface PmBox {
  id: number;
  name: string;
  count: number;
}

/** 表单输入统一样式（写信/搜索共用的 44px 触控规格） */
export const inputCls =
  "min-h-[44px] w-full rounded-[var(--r-sm)] border border-[var(--baozi-line)] bg-[var(--baozi-paper)] px-3 text-sm text-ink outline-none focus:border-[var(--baozi-orange)]";

/** 消息列表表格（从 components/message-center.tsx 按域拆出）：
 *  收/发件视图行渲染——未读高亮、系统通知 🔔、展开详情（markdown 渲染 +
 *  回复/转发按钮）、全选与单选。数据与动作回调由 MessageCenter 注入。 */
export function MessageTable({
  box,
  rows,
  selected,
  onToggle,
  openId,
  onOpen,
  onReply,
  onForward,
}: {
  box: "inbox" | "sent";
  rows: MessageRow[];
  selected: number[];
  onToggle: (id: number, on: boolean) => void;
  openId: number | null;
  onOpen: (m: MessageRow) => void;
  onReply: (m: MessageRow) => void;
  onForward: (m: MessageRow) => void;
}) {
  const { dict, locale } = useI18n();
  const t = dict.messages;
  return (
    <table className="nexus-table">
      <thead>
        <tr>
          <th className="hidden w-10 sm:table-cell">
            <span className="sr-only">{t.selectAll}</span>
          </th>
          <th className="hidden w-16 sm:table-cell">{t.readCol}</th>
          <th>{t.subject}</th>
          <th className="hidden sm:table-cell">
            {box === "inbox" ? t.fromCol : t.toCol}
          </th>
          <th className="hidden w-44 md:table-cell">{t.timeCol}</th>
          <th className="hidden w-16 text-right sm:table-cell">
            {t.actionCol}
          </th>
        </tr>
      </thead>
      <tbody>
        {rows.map((m) => {
          const cp = m.counterpart ?? t.systemSender;
          const time = new Date(m.created_at).toLocaleString(
            dateLocale(locale),
          );
          const unread = box === "inbox" && (m.unread ?? !m.read_at);
          const status =
            box === "inbox" ? (unread ? t.unreadTag : t.readTag) : t.sentTag;
          return (
            <tr key={m.id}>
              <td className="hidden sm:table-cell">
                <input
                  type="checkbox"
                  checked={selected.includes(m.id)}
                  onChange={(e) => onToggle(m.id, e.target.checked)}
                />
              </td>
              <td className="hidden sm:table-cell">
                <span aria-hidden className="mr-1">
                  {box === "sent"
                    ? "📤"
                    : m.is_system
                      ? "🔔"
                      : unread
                        ? "📬"
                        : "📭"}
                </span>
                <span
                  className={`text-[11px] ${unread ? "font-bold text-[var(--baozi-orange-dark)]" : "text-sub"}`}
                >
                  {status}
                </span>
              </td>
              <td className="min-w-0">
                <button
                  type="button"
                  onClick={() => onOpen(m)}
                  className={`block max-w-full truncate text-left ${unread ? "font-bold text-[var(--baozi-orange-dark)]" : "text-ink"}`}
                >
                  {box === "inbox" && m.is_system && (
                    <span aria-hidden className="mr-1" title={t.systemSender}>
                      🔔
                    </span>
                  )}
                  {m.subject}
                </button>
                {openId === m.id && (
                  <div className="mt-2 rounded-[var(--r-sm)] border border-dashed border-[var(--baozi-line)] bg-[var(--baozi-cream)] p-2">
                    <div className="text-sm">
                      <MarkdownRenderer source={m.body} />
                    </div>
                    <p className="mt-1 text-[11px] text-sub md:hidden">
                      {cp} · {time}
                    </p>
                    {/* 系统通知没有对端用户，回复/转发无意义（0123 视觉区分的一部分） */}
                    {!m.is_system && box === "inbox" && (
                      <div className="mt-2 flex justify-end gap-2">
                        <button
                          type="button"
                          onClick={() => onReply(m)}
                          className="min-h-[32px] rounded-full border border-[var(--baozi-orange-dark)] px-3 text-xs font-bold text-[var(--baozi-orange-dark)] hover:bg-[var(--baozi-orange)] hover:text-white"
                        >
                          ↩ {t.reply}
                        </button>
                        <button
                          type="button"
                          onClick={() => onForward(m)}
                          className="min-h-[32px] rounded-full border border-[var(--baozi-line)] px-3 text-xs font-bold text-sub hover:text-ink"
                        >
                          ↪ {t.forward}
                        </button>
                      </div>
                    )}
                  </div>
                )}
              </td>
              <td className="hidden text-sky sm:table-cell">{cp}</td>
              <td className="hidden text-[11px] text-sub md:table-cell">
                {time}
              </td>
                            <td
                  className="hidden text-right sm:table-cell"
                >   <button
                  type="button"
                  onClick={() => onOpen(m)}
                  className="text-xs font-bold text-sky hover:text-[var(--baozi-orange)]"
                >
                  {openId === m.id ? t.backToList : t.view}
                </button>
              </td>
            </tr>
          );
        })}
      </tbody>
    </table>
  );
}
