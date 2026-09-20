"use client";

import { useCallback, useEffect, useState } from "react";
import { api } from "@/lib/api-client";
import { useI18n } from "@/i18n/client";
import { dateLocale } from "@/i18n/config";

/** 管理组信箱共享契约（从 components/staff-box.tsx 按域拆出） */

export interface MessageRow {
  id: number;
  counterpart: string | null;
  subject: string;
  body: string;
  read_at: string | null;
  created_at: string;
}

export interface StaffMsg {
  id: number;
  username: string | null;
  subject: string;
  body: string;
  answered: number;
  answered_by: string | null;
  answer: string | null;
  answered_at: string | null;
  created_at: string;
}

/** 工单四态（0078）：GET /stafftickets?status= + POST /stafftickets/update */
export interface TicketRow {
  id: number;
  username: string | null;
  subject: string;
  priority: number;
  assigned_to: string | null;
  ticket_status: number; // 0新 1处理中 2已答复待确认 3关闭
  created_at: string;
  answered_at: string | null;
}

/** 咨询工作台表格（从 components/staff-box.tsx 按域拆出）：
 *  未答复/已答复两态列表——详情展开、答复输入（未答复态）、批量勾选。
 *  数据与动作由 StaffBox 注入。 */
export function DeskTable({
  deskRows,
  deskTab,
  deskSel,
  onToggle,
  answeringId,
  answerText,
  setAnsweringId,
  setAnswerText,
  busy,
  onSendAnswer,
}: {
  deskRows: StaffMsg[] | null;
  deskTab: 0 | 1;
  deskSel: number[];
  onToggle: (id: number, on: boolean) => void;
  answeringId: number | null;
  answerText: string;
  setAnsweringId: (id: number) => void;
  setAnswerText: (v: string) => void;
  busy: boolean;
  onSendAnswer: (id: number) => void;
}) {
  const { dict, locale } = useI18n();
  const t = dict.staffbox;
  return (
    <table className="nexus-table">
      <thead>
        <tr>
          <th className="w-10" />
          <th>{dict.messages.subject}</th>
          <th className="w-24">{t.from ?? "来信人"}</th>
          <th className="hidden w-40 md:table-cell">{dict.messages.timeCol}</th>
          {deskTab === 1 && <th className="w-24">{t.answeredBy ?? "答复人"}</th>}
        </tr>
      </thead>
      <tbody>
        {(deskRows ?? []).map((m) => (
          <tr key={m.id}>
            <td>
              <input
                type="checkbox"
                checked={deskSel.includes(m.id)}
                onChange={(e) => onToggle(m.id, e.target.checked)}
              />
            </td>
            <td>
              <details>
                <summary className="cursor-pointer font-bold">{m.subject}</summary>
                <p className="mt-2 whitespace-pre-wrap rounded-[var(--r-sm)] border border-dashed border-[var(--baozi-line)] bg-[var(--baozi-cream)] p-2 text-sm">
                  {m.body}
                </p>
                {m.answer && (
                  <p className="mt-1 whitespace-pre-wrap text-sm text-sub">↩ {m.answer}</p>
                )}
                {deskTab === 0 && (
                  <div className="mt-2 flex flex-col gap-2">
                    <textarea
                      rows={3}
                      value={answeringId === m.id ? answerText : ""}
                      onChange={(e) => {
                        setAnsweringId(m.id);
                        setAnswerText(e.target.value);
                      }}
                      placeholder={t.answerPh ?? "输入答复，将私信发给来信人并回写工单"}
                      className="min-h-[70px] rounded-[var(--r-sm)] border border-[var(--baozi-line)] bg-[var(--baozi-paper)] px-3 py-2 text-sm"
                    />
                    <button
                      type="button"
                      disabled={busy || (answeringId === m.id && !answerText.trim())}
                      onClick={() => onSendAnswer(m.id)}
                      className="min-h-[36px] w-fit rounded-full bg-sky-deep px-4 text-xs font-bold text-white disabled:opacity-50"
                    >
                      {t.sendBtn ?? "发送答复"}
                    </button>
                  </div>
                )}
              </details>
            </td>
            <td className="text-sky">{m.username ?? "—"}</td>
            <td className="hidden text-[11px] text-sub md:table-cell">
              {new Date(m.created_at).toLocaleString(dateLocale(locale))}
            </td>
            {deskTab === 1 && <td className="text-[11px]">{m.answered_by ?? "—"}</td>}
          </tr>
        ))}
        {deskRows !== null && deskRows.length === 0 && (
          <tr>
            <td colSpan={5} className="py-6 text-center text-sub">
              {t.emptyDesk ?? "暂无来信"}
            </td>
          </tr>
        )}
      </tbody>
    </table>
  );
}

/** 工单视图状态装载（从 components/staff-box.tsx 按域拆出）：
 *  按状态拉取 stafftickets，四态标签映射 tk.stNew/stProgress/…。 */
export function useTickets(status: string) {
  const [ticketRows, setTicketRows] = useState<TicketRow[] | null>(null);
  const loadTickets = useCallback(() => {
    const qs = status ? `?status=${status}` : "";
    api
      .get<TicketRow[]>(`/api/v1/stafftickets${qs}`)
      .then(setTicketRows)
      .catch(() => setTicketRows([]));
  }, [status]);
  useEffect(() => {
    loadTickets();
  }, [loadTickets]);
  return { ticketRows, loadTickets };
}

export function ticketStatusLabel(
  tk: ReturnType<typeof useI18n>["dict"]["ticket"],
  s: number,
): string {
  return [tk.stNew, tk.stProgress, tk.stAnswered, tk.stClosed][s] ?? String(s);
}
