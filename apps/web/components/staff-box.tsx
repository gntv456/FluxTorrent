"use client";

import { useCallback, useEffect, useState } from "react";
import { api, ApiError } from "@/lib/api-client";
import { useI18n } from "@/i18n/client";
import {
  DeskTable,
  useTickets,
  type MessageRow,
  type StaffMsg,
} from "@/components/staff-box-desk";
import { TicketPanel } from "@/components/staff-box-tickets";
import { PmList, PmReplyForm } from "@/components/staff-box-pm";

// 管理组信箱（好学站 staffbox.php 口径）：与管理组成员的往来短讯，支持回复
// 0078 工单化：新增「工单」视图（stafftickets 四态流转 + 指派 + 优先级）。
// 拆出：契约/咨询工作台表格 @/components/staff-box-desk、
// 工单面板 @/components/staff-box-tickets、私信视图 @/components/staff-box-pm。

export function StaffBox() {
  const { dict } = useI18n();
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
  const { ticketRows, loadTickets } = useTickets(ticketStatus);
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
              ["desk", t.deskTab],
              ["ticket", tk.tab],
              ["pm", t.pmTab],
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
                {t.pending}
              </button>
              <button
                type="button"
                onClick={() => setDeskTab(1)}
                className={`min-h-[36px] rounded-full px-3 text-xs font-bold ${deskTab === 1 ? "text-[var(--baozi-orange-dark)] underline" : "text-sub"}`}
              >
                {t.answered}
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
              ✓ {t.mark}
            </button>
            <button
              type="button"
              disabled={deskSel.length === 0 || busy}
              onClick={() => bulkDesk("delete")}
              className="min-h-[36px] rounded-full border border-[var(--danger-border)] px-3 font-bold text-danger disabled:opacity-40"
            >
              🗑 {t.del}
            </button>
          </div>
        )}
      </div>

      {/* ① 咨询工作台（staffmessages） */}
      {view === "desk" && (
        <DeskTable
          deskRows={deskRows}
          deskTab={deskTab}
          deskSel={deskSel}
          onToggle={(id, on) =>
            setDeskSel((s2) => (on ? [...s2, id] : s2.filter((x) => x !== id)))
          }
          answeringId={answeringId}
          answerText={answerText}
          setAnsweringId={setAnsweringId}
          setAnswerText={setAnswerText}
          busy={busy}
          onSendAnswer={(id) => void sendAnswer(id)}
        />
      )}

      {/* ③ 工单（stafftickets 四态流转） */}
      {view === "ticket" && (
        <TicketPanel
          tk={tk}
          ticketStatus={ticketStatus}
          setTicketStatus={setTicketStatus}
          ticketRows={ticketRows}
          ticketMsg={ticketMsg}
          ticketEdit={ticketEdit}
          onEditToggle={(id) => {
            setTicketEdit(ticketEdit === id ? null : id);
            setTicketForm({ priority: "", ticket_status: "", assign: "" });
          }}
          ticketForm={ticketForm}
          setTicketForm={setTicketForm}
          ticketBusy={ticketBusy}
          onUpdate={(id) => void updateTicket(id)}
        />
      )}

      {err && <p className="funbox__empty">{err}</p>}
      {!err && rows === null && <p className="funbox__empty">…</p>}
      {!err && rows !== null && rows.length === 0 && <p className="funbox__empty">{t.empty}</p>}
      {msg && <p className="funbox__msg">{msg}</p>}

      {view === "pm" && rows !== null && rows.length > 0 && (
        <PmList
          rows={rows}
          openId={openId}
          onToggleOpen={(id) => setOpenId(openId === id ? null : id)}
          onReply={(m) => {
            setReplyTo(m);
            setReplyBody("");
          }}
        />
      )}

      {/* 回复表单 */}
      {view === "pm" && replyTo && (
        <PmReplyForm
          replyTo={replyTo}
          replyBody={replyBody}
          setReplyBody={setReplyBody}
          busy={busy}
          onSubmit={() => void sendReply()}
          onClose={() => setReplyTo(null)}
        />
      )}
    </section>
  );
}
