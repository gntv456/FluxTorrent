"use client";

import dynamic from "next/dynamic";
import { useI18n } from "@/i18n/client";
import { dateLocale } from "@/i18n/config";
import { noticeText } from "@/components/message-center-notice";
import type { MessageRow } from "@/components/message-center-table";

// W-33 代码分割：markdown-lite 渲染器（含视频内嵌客户端件）只在
// 展开消息正文时才需要，延迟加载 + 行内骨架。
const MarkdownRenderer = dynamic(
  () => import("@/components/forum-markdown").then((m) => m.MarkdownRenderer),
  { loading: () => <span className="text-xs text-sub">…</span> },
);

/** 卡片壳（与站内其它卡片同底） */
const CARD_CLS =
  "rounded-[var(--r-md)] border border-[var(--brand-line)] " +
  "bg-[var(--brand-paper)] p-3";
const DOT_CLS =
  "mt-1.5 h-2 w-2 shrink-0 rounded-full bg-[var(--brand-orange)]";
const BODY_CLS =
  "mt-2 rounded-[var(--r-sm)] border border-dashed " +
  "border-[var(--brand-line)] bg-[var(--brand-cream)] p-2";
const BTN_REPLY =
  "min-h-[32px] rounded-full border border-[var(--brand-orange-dark)] " +
  "px-3 text-xs font-bold text-[var(--brand-orange-dark)]";
const BTN_FWD =
  "min-h-[32px] rounded-full border border-[var(--brand-line)] " +
  "px-3 text-xs font-bold text-sub";

/** 消息列表的**手机卡片**形态（2026-11-02）。
 *
 *  桌面是六列表格（勾选/状态/主题/发件人/时间/操作）；窄屏那张表把六列
 *  挤进 390px，只剩「主题」一列可读——发件人与时间被藏进展开区，等于
 *  「每条消息是谁发的、什么时候发的」在列表上根本看不到。
 *  卡片把这两项摆回第一层：未读点 + 主题 + 对端 + 时间 + 摘要。
 */
export function MessageCards({
  box,
  rows,
  openId,
  onOpen,
  onReply,
  onForward,
}: {
  box: "inbox" | "sent";
  rows: MessageRow[];
  openId: number | null;
  onOpen: (m: MessageRow) => void;
  onReply: (m: MessageRow) => void;
  onForward: (m: MessageRow) => void;
}) {
  const { dict, locale } = useI18n();
  const t = dict.messages;
  return (
    <ul className="flex flex-col gap-2 sm:hidden">
      {rows.map((m) => {
        const cp = m.counterpart ?? t.systemSender;
        const time = new Date(m.created_at).toLocaleString(
          dateLocale(locale),
          {
            month: "numeric",
            day: "numeric",
            hour: "2-digit",
            minute: "2-digit",
          },
        );
        const unread = box === "inbox" && (m.unread ?? !m.read_at);
        return (
          <li key={m.id} className={CARD_CLS}>
            <div className="flex items-start gap-2">
              {unread ? (
                <span aria-hidden className={DOT_CLS} />
              ) : (
                <span aria-hidden className="mt-1.5 w-2 shrink-0" />
              )}
              <button
                type="button"
                onClick={() => onOpen(m)}
                className={
                  "min-w-0 flex-1 text-left text-[13px] leading-snug " +
                  (unread
                    ? "font-bold text-[var(--brand-orange-dark)]"
                    : "text-ink")
                }
              >
                {box === "inbox" && m.is_system && (
                  <span aria-hidden className="mr-1" title={t.systemSender}>
                    🔔
                  </span>
                )}
                {noticeText(t, m, "s", m.subject)}
              </button>
            </div>
            <p className="mt-1 pl-4 text-[11px] text-sub">
              {cp}
              {" · "}
              {time}
            </p>
            {openId === m.id && (
              <div className={BODY_CLS}>
                <div className="text-sm">
                  <MarkdownRenderer source={noticeText(t, m, "b", m.body)} />
                </div>
                {!m.is_system && box === "inbox" && (
                  <div className="mt-2 flex justify-end gap-2">
                    <button
                      type="button"
                      onClick={() => onReply(m)}
                      className={BTN_REPLY}
                    >
                      ↩ {t.reply}
                    </button>
                    <button
                      type="button"
                      onClick={() => onForward(m)}
                      className={BTN_FWD}
                    >
                      ↪ {t.forward}
                    </button>
                  </div>
                )}
              </div>
            )}
          </li>
        );
      })}
    </ul>
  );
}
