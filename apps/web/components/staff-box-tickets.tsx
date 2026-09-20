"use client";

import { useI18n } from "@/i18n/client";
import { dateLocale } from "@/i18n/config";
import { ticketStatusLabel, type TicketRow } from "@/components/staff-box-desk";

/** 工单面板（从 components/staff-box.tsx 按域拆出）：
 *  状态筛选 chip 行 + 八列工单表（优先级/指派/四态徽标）+ 编辑行
 *  （优先级/状态/指派任意子集更新）。数据与动作由 StaffBox 注入。 */

export interface TicketForm {
  priority: string;
  ticket_status: string;
  assign: string;
}

export function TicketPanel({
  tk,
  ticketStatus,
  setTicketStatus,
  ticketRows,
  ticketMsg,
  ticketEdit,
  onEditToggle,
  ticketForm,
  setTicketForm,
  ticketBusy,
  onUpdate,
}: {
  tk: ReturnType<typeof useI18n>["dict"]["ticket"];
  ticketStatus: string;
  setTicketStatus: (s: string) => void;
  ticketRows: TicketRow[] | null;
  ticketMsg: string | null;
  ticketEdit: number | null;
  onEditToggle: (id: number) => void;
  ticketForm: TicketForm;
  setTicketForm: (f: TicketForm) => void;
  ticketBusy: boolean;
  onUpdate: (id: number) => void;
}) {
  const { dict, locale } = useI18n();
  const t = dict.staffbox;
  return (
    <section className="flex flex-col gap-2">
      <div className="flex flex-wrap items-center gap-1">
        {(["", "0", "1", "2", "3"] as const).map((s) => (
          <button
            key={s}
            type="button"
            onClick={() => setTicketStatus(s)}
            className={`min-h-[32px] rounded-full px-3 text-xs font-bold ${
              ticketStatus === s ? "text-[var(--baozi-orange-dark)] underline" : "text-sub"
            }`}
          >
            {s === "" ? tk.statusAll : ticketStatusLabel(tk, Number(s))}
          </button>
        ))}
      </div>
      {ticketMsg && (
        <p className="rounded-[var(--r-md)] bg-sky-soft p-2 text-xs text-ink" role="status">
          {ticketMsg}
        </p>
      )}
      <div className="baozi-wide-table-scroll">
        <table className="nexus-table text-xs">
          <thead>
            <tr>
              <td className="colhead w-12">#</td>
              <td className="colhead">{dict.messages.subject}</td>
              <td className="colhead w-24">{t.from ?? "来信人"}</td>
              <td className="colhead w-20">{tk.priority}</td>
              <td className="colhead w-28">{tk.assignedTo}</td>
              <td className="colhead w-28">{tk.statusLabel}</td>
              <td className="colhead w-40">{dict.messages.timeCol}</td>
              <td className="colhead w-24" />
            </tr>
          </thead>
          <tbody>
            {(ticketRows ?? []).map((m) => (
              <tr key={m.id}>
                <td className="num">{m.id}</td>
                <td className="font-bold">{m.subject}</td>
                <td className="text-sky">{m.username ?? "—"}</td>
                <td>{tk.priorityNames[m.priority] ?? m.priority}</td>
                <td className="text-sub">{m.assigned_to ?? tk.unassigned}</td>
                <td>
                  <span
                    className={`rounded-full px-2 py-0.5 text-[10px] font-bold ${
                      m.ticket_status === 0
                        ? "bg-sun/30"
                        : m.ticket_status === 1
                          ? "bg-sky-soft"
                          : m.ticket_status === 2
                            ? "bg-mint/30"
                            : "bg-[var(--surface-raised)] text-sub"
                    }`}
                  >
                    {ticketStatusLabel(tk, m.ticket_status)}
                  </span>
                </td>
                <td className="text-[11px] text-sub">
                  {new Date(m.created_at).toLocaleString(dateLocale(locale))}
                </td>
                <td className="text-right">
                  <button
                    type="button"
                    onClick={() => onEditToggle(m.id)}
                    className="min-h-[28px] rounded-full border border-line px-3 text-[11px] font-bold"
                  >
                    {tk.updateBtn}
                  </button>
                </td>
              </tr>
            ))}
            {ticketRows !== null && ticketRows.length === 0 && (
              <tr>
                <td colSpan={8} className="py-6 text-center text-sub">
                  {tk.empty}
                </td>
              </tr>
            )}
          </tbody>
        </table>
      </div>

      {/* 工单编辑行：优先级 / 状态 / 指派（任意子集） */}
      {ticketEdit !== null && (
        <div className="baozi-panel flex flex-col gap-2 p-3">
          <p className="text-xs font-bold text-sub">
            #{ticketEdit} · {tk.updateBtn}
          </p>
          <div className="flex flex-wrap items-end gap-2">
            <label className="flex flex-col gap-1 text-xs">
              {tk.priority}
              <select
                value={ticketForm.priority}
                onChange={(e) => setTicketForm({ ...ticketForm, priority: e.target.value })}
                className="min-h-[36px] rounded-[var(--r-sm)] border border-line bg-cloud px-2"
              >
                <option value="">—</option>
                {tk.priorityNames.map((p, i) => (
                  <option key={i} value={String(i)}>
                    {i} {p}
                  </option>
                ))}
              </select>
            </label>
            <label className="flex flex-col gap-1 text-xs">
              {tk.statusLabel}
              <select
                value={ticketForm.ticket_status}
                onChange={(e) => setTicketForm({ ...ticketForm, ticket_status: e.target.value })}
                className="min-h-[36px] rounded-[var(--r-sm)] border border-line bg-cloud px-2"
              >
                <option value="">—</option>
                <option value="1">{tk.setProgress}</option>
                <option value="2">{tk.setAnswered}</option>
                <option value="3">{tk.closeBtn}</option>
              </select>
            </label>
            <label className="flex flex-col gap-1 text-xs">
              {tk.assignedTo}
              <input
                type="text"
                value={ticketForm.assign}
                onChange={(e) => setTicketForm({ ...ticketForm, assign: e.target.value })}
                placeholder={tk.assignPh}
                className="min-h-[36px] w-48 rounded-[var(--r-sm)] border border-line bg-cloud px-2"
              />
            </label>
            <button
              type="button"
              disabled={ticketBusy}
              onClick={() => onUpdate(ticketEdit)}
              className="baozi-button min-h-[36px] text-xs disabled:opacity-50"
            >
              {tk.updateBtn}
            </button>
          </div>
        </div>
      )}
    </section>
  );
}
