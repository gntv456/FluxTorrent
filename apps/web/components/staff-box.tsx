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

interface StaffMsg {
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
interface TicketRow {
  id: number;
  username: string | null;
  subject: string;
  priority: number;
  assigned_to: string | null;
  ticket_status: number; // 0新 1处理中 2已答复待确认 3关闭
  created_at: string;
  answered_at: string | null;
}

/** 管理组信箱（好学站 staffbox.php 口径）：与管理组成员的往来短讯，支持回复
 *  0078 工单化：新增「工单」视图（stafftickets 四态流转 + 指派 + 优先级） */
export function StaffBox() {
  const { dict, locale } = useI18n();
  const t = dict.staffbox;
  const tk = dict.ticket;
  const [view, setView] = useState<"desk" | "pm" | "ticket">("desk");
  const [deskTab, setDeskTab] = useState<0 | 1>(0);
  const [deskRows, setDeskRows] = useState<StaffMsg[] | null>(null);
  const [deskSel, setDeskSel] = useState<number[]>([]);
  const [answeringId, setAnsweringId] = useState<number | null>(null);
  const [answerText, setAnswerText] = useState("");
  // 工单视图
  const [ticketStatus, setTicketStatus] = useState<string>(""); // ""=全部
  const [ticketRows, setTicketRows] = useState<TicketRow[] | null>(null);
  const [ticketMsg, setTicketMsg] = useState<string | null>(null);
  const [ticketBusy, setTicketBusy] = useState(false);
  const [ticketEdit, setTicketEdit] = useState<number | null>(null);
  const [ticketForm, setTicketForm] = useState({ priority: "", ticket_status: "", assign: "" });
  const [rows, setRows] = useState<MessageRow[] | null>(null);
  const [err, setErr] = useState<string | null>(null);
  const [openId, setOpenId] = useState<number | null>(null);
  const [replyTo, setReplyTo] = useState<MessageRow | null>(null);
  const [replyBody, setReplyBody] = useState("");
  const [busy, setBusy] = useState(false);
  const [msg, setMsg] = useState<string | null>(null);

  const load = () => {
    api
      .get<StaffMsg[]>(`/api/v1/staffmessages?answered=${deskTab}`)
      .then((r) => {
        setDeskRows(r);
        setDeskSel([]);
      })
      .catch(() => setDeskRows([]));
    api
      .get<MessageRow[]>("/api/v1/messages/staff")
      .then(setRows)
      .catch((e) =>
        setErr(e instanceof ApiError ? (dict.errors[e.code] ?? e.message) : dict.common.loadFailed),
      );
  };

  useEffect(() => {
    load();
    // eslint-disable-next-line react-hooks/exhaustive-deps
  }, [dict, deskTab]);

  const loadTickets = useCallback(() => {
    const qs = ticketStatus ? `?status=${ticketStatus}` : "";
    api
      .get<TicketRow[]>(`/api/v1/stafftickets${qs}`)
      .then(setTicketRows)
      .catch(() => setTicketRows([]));
  }, [ticketStatus]);
  useEffect(() => {
    if (view === "ticket") loadTickets();
  }, [view, loadTickets]);

  async function updateTicket(id: number) {
    setTicketBusy(true);
    setTicketMsg(null);
    try {
      await api.post("/api/v1/stafftickets/update", {
        id,
        priority: ticketForm.priority !== "" ? Number(ticketForm.priority) : undefined,
        ticket_status: ticketForm.ticket_status !== "" ? Number(ticketForm.ticket_status) : undefined,
        assign: ticketForm.assign,
      });
      setTicketMsg(tk.updated);
      setTicketEdit(null);
      setTicketForm({ priority: "", ticket_status: "", assign: "" });
      loadTickets();
    } catch (e) {
      setTicketMsg(e instanceof ApiError ? e.message : dict.common.networkError);
    } finally {
      setTicketBusy(false);
    }
  }

  const ticketStatusLabel = (s: number) =>
    [tk.stNew, tk.stProgress, tk.stAnswered, tk.stClosed][s] ?? String(s);

  async function sendAnswer(id: number) {
    if (!answerText.trim()) return;
    setBusy(true);
    setMsg(null);
    try {
      await api.post("/api/v1/staffmessages/answer", { id, answer: answerText.trim() });
      setMsg(dict.messages.sentOk);
      setAnsweringId(null);
      setAnswerText("");
      load();
    } catch (e) {
      setMsg(e instanceof ApiError ? e.message : dict.common.networkError);
    } finally {
      setBusy(false);
    }
  }

  async function bulkDesk(action: "mark" | "delete") {
    if (deskSel.length === 0) return;
    setBusy(true);
    try {
      await api.post(`/api/v1/staffmessages/${action}`, { ids: deskSel });
      load();
    } catch (e) {
      setMsg(e instanceof ApiError ? e.message : dict.common.networkError);
    } finally {
      setBusy(false);
    }
  }

  async function sendReply() {
    if (!replyTo || !replyBody.trim()) return;
    setBusy(true);
    setMsg(null);
    try {
      await api.post("/api/v1/messages", {
        to: replyTo.counterpart ?? "root",
        subject: /^re:/i.test(replyTo.subject) ? replyTo.subject : `Re: ${replyTo.subject}`,
        body: replyBody.trim(),
      });
      setReplyTo(null);
      setReplyBody("");
      setMsg(dict.messages.sentOk);
      load();
    } catch (e) {
      setMsg(e instanceof ApiError ? e.message : dict.common.networkError);
    } finally {
      setBusy(false);
    }
  }

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

      {/* 视图切换 + 工作台批量操作 */}
      <div className="flex flex-wrap items-center justify-between gap-2">
        <div className="flex flex-wrap gap-2">
          {(
            [
              ["desk", t.deskTab ?? "咨询工作台"],
              ["ticket", tk.tab],
              ["pm", t.pmTab ?? "管理组私信"],
            ] as ["desk" | "ticket" | "pm", string][]
          ).map(([k, label]) => (
            <button
              key={k}
              type="button"
              onClick={() => setView(k)}
              aria-current={view === k ? "true" : undefined}
              className={`min-h-[44px] rounded-[10px] border px-4 text-sm font-bold ${
                view === k
                  ? "border-[var(--baozi-orange)] bg-[linear-gradient(135deg,var(--baozi-orange-bright),var(--baozi-orange))] text-white"
                  : "border-[var(--baozi-line)] bg-[var(--row-veil-soft)] text-ink"
              }`}
            >
              {label}
            </button>
          ))}
          {view === "desk" && (
            <span className="flex items-center gap-1">
              <button
                type="button"
                onClick={() => setDeskTab(0)}
                className={`min-h-[36px] rounded-full px-3 text-xs font-bold ${deskTab === 0 ? "text-[var(--baozi-orange-dark)] underline" : "text-sub"}`}
              >
                {t.pending ?? "未答复"}
              </button>
              <button
                type="button"
                onClick={() => setDeskTab(1)}
                className={`min-h-[36px] rounded-full px-3 text-xs font-bold ${deskTab === 1 ? "text-[var(--baozi-orange-dark)] underline" : "text-sub"}`}
              >
                {t.answered ?? "已答复"}
              </button>
            </span>
          )}
        </div>
        {view === "desk" && (
          <div className="flex gap-2 text-xs">
            <button
              type="button"
              disabled={deskSel.length === 0 || busy}
              onClick={() => bulkDesk("mark")}
              className="min-h-[36px] rounded-full border border-[var(--baozi-line)] px-3 font-bold disabled:opacity-40"
            >
              ✓ {t.mark ?? "标记已答复"}
            </button>
            <button
              type="button"
              disabled={deskSel.length === 0 || busy}
              onClick={() => bulkDesk("delete")}
              className="min-h-[36px] rounded-full border border-[var(--danger-border)] px-3 font-bold text-danger disabled:opacity-40"
            >
              🗑 {t.del ?? "删除"}
            </button>
          </div>
        )}
      </div>

      {/* ① 咨询工作台（staffmessages） */}
      {view === "desk" && (
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
                    onChange={(e) =>
                      setDeskSel((s2) =>
                        e.target.checked ? [...s2, m.id] : s2.filter((x) => x !== m.id),
                      )
                    }
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
                          onClick={() => sendAnswer(m.id)}
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
      )}

      {/* ③ 工单（stafftickets 四态流转） */}
      {view === "ticket" && (
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
                {s === "" ? tk.statusAll : ticketStatusLabel(Number(s))}
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
                        {ticketStatusLabel(m.ticket_status)}
                      </span>
                    </td>
                    <td className="text-[11px] text-sub">
                      {new Date(m.created_at).toLocaleString(dateLocale(locale))}
                    </td>
                    <td className="text-right">
                      <button
                        type="button"
                        onClick={() => {
                          setTicketEdit(ticketEdit === m.id ? null : m.id);
                          setTicketForm({ priority: "", ticket_status: "", assign: "" });
                        }}
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
                  onClick={() => updateTicket(ticketEdit)}
                  className="baozi-button min-h-[36px] text-xs disabled:opacity-50"
                >
                  {tk.updateBtn}
                </button>
              </div>
            </div>
          )}
        </section>
      )}

      {err && <p className="funbox__empty">{err}</p>}
      {!err && rows === null && <p className="funbox__empty">…</p>}
      {!err && rows !== null && rows.length === 0 && <p className="funbox__empty">{t.empty}</p>}
      {msg && <p className="funbox__msg">{msg}</p>}

      {view === "pm" && rows !== null && rows.length > 0 && (
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
                <div className="border-t border-dashed border-[var(--border-soft)] px-4 py-3">
                  <p className="whitespace-pre-wrap text-sm text-ink">{m.body}</p>
                  <div className="mt-2 flex justify-end">
                    <button
                      type="button"
                      onClick={() => {
                        setReplyTo(m);
                        setReplyBody("");
                      }}
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
      )}

      {/* 回复表单 */}
      {view === "pm" && replyTo && (
        <section className="baozi-panel">
          <header className="baozi-panel__head">
            <h2>
              ↩ {dict.messages.reply} · {replyTo.counterpart ?? "Staff"} — {replyTo.subject}
            </h2>
            <button type="button" className="min-h-[32px] rounded-full border border-line px-3 text-xs font-bold" onClick={() => setReplyTo(null)}>
              ✕
            </button>
          </header>
          <form
            className="flex flex-col gap-3 px-4 pb-4"
            onSubmit={(e) => {
              e.preventDefault();
              void sendReply();
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
      )}
    </section>
  );
}
