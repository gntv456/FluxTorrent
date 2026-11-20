"use client";

import { useCallback, useEffect, useState } from "react";
import { api, ApiError } from "@/lib/api-client";
import { apiErrorMessage, useI18n } from "@/i18n/client";
import {
  DeskTable,
  useTickets,
  type MessageRow,
  type StaffMsg,
} from "@/components/staff-box-desk";
import { TicketPanel } from "@/components/staff-box-tickets";
import { useStaffTickets } from "@/components/staff-box-ticket-hook";
import { PmList, PmReplyForm } from "@/components/staff-box-pm";
import { ErrorPanel } from "@/components/ui/error-panel";
import { ConfirmDialog } from "@/components/ui/confirm-dialog";
import { fmt } from "@/i18n/config";

// 管理组信箱（staffbox.php 口径）：与管理组成员的往来短讯+回复。
// 0078 工单化：工单视图（stafftickets 四态流转+指派+优先级）。
// 拆件：desk（契约/咨询表格）/tickets/pm 三子模块。

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
  const tkState = useStaffTickets(tk.updated, dict.common.networkError);
  const {
    ticketStatus, setTicketStatus, ticketRows, ticketErr, loadTickets,
    ticketMsg, ticketBusy, ticketEdit, setTicketEdit, ticketForm, setTicketForm,
    updateTicket: updateTicketRaw,
  } = tkState;
  const [rows, setRows] = useState<MessageRow[] | null>(null);
  const [deskErr, setDeskErr] = useState<unknown>(null);
  const [err, setErr] = useState<string | null>(null);
  const [openId, setOpenId] = useState<number | null>(null);
  const [replyTo, setReplyTo] = useState<MessageRow | null>(null);
  const [replyBody, setReplyBody] = useState("");
  const [busy, setBusy] = useState(false);
  const [msg, setMsg] = useState<string | null>(null);
  // P1：批量删除不可逆，需二次确认 + 数量提示
  const [askBulk, setAskBulk] = useState<"mark" | "delete" | null>(null);

  const load = () => {
    setDeskErr(null);
    api
      .get<StaffMsg[]>(`/api/v1/staffmessages?answered=${deskTab}`)
      .then((r) => { setDeskRows(r); setDeskSel([]); })
      .catch((e: unknown) => { setDeskErr(e); setDeskRows([]); });
    api
      .get<MessageRow[]>("/api/v1/messages/staff")
      .then(setRows)
      .catch((e) =>
        setErr(e instanceof ApiError ? (apiErrorMessage(dict, e)) : dict.common.loadFailed),
      );
  };

  useEffect(() => {
    load();
    // eslint-disable-next-line react-hooks/exhaustive-deps
  }, [dict, deskTab]);

  useEffect(() => {
    if (view === "ticket") loadTickets();
  }, [view, loadTickets]);

  const updateTicket = (id: number) => updateTicketRaw(id);

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
      // 成功反馈（此前零提示，玩家只能靠列表刷新自行比对）
      setMsg(fmt(dict.review.modDone, {}) + `（${deskSel.length}）`);
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
          className="min-h-[44px] rounded-[10px] border border-[var(--compat-orange-dark)] bg-[linear-gradient(135deg,var(--compat-orange-bright),var(--compat-orange))] px-4 text-sm font-bold leading-[44px] text-white shadow-[var(--shadow-hover)] active:scale-[0.97]"
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
                  ? "border-[var(--compat-orange)] bg-[linear-gradient(135deg,var(--compat-orange-bright),var(--compat-orange))] text-white"
                  : "border-[var(--compat-line)] bg-[var(--row-veil-soft)] text-ink"
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
                className={`min-h-11 rounded-full px-3 text-xs font-bold ${deskTab === 0 ? "text-[var(--compat-orange-dark)] underline" : "text-sub"}`}
              >
                {t.pending}
              </button>
              <button
                type="button"
                onClick={() => setDeskTab(1)}
                className={`min-h-11 rounded-full px-3 text-xs font-bold ${deskTab === 1 ? "text-[var(--compat-orange-dark)] underline" : "text-sub"}`}
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
              onClick={() => setAskBulk("mark")}
              className="min-h-11 rounded-full border border-[var(--compat-line)] px-3 font-bold disabled:opacity-40"
            >
              ✓ {t.mark}
            </button>
            <button
              type="button"
              disabled={deskSel.length === 0 || busy}
              onClick={() => setAskBulk("delete")}
              className="min-h-11 rounded-full border border-[var(--danger-border)] px-3 font-bold text-[var(--danger-text)] disabled:opacity-40"
            >
              🗑 {t.del}
            </button>
          </div>
        )}
      </div>

      {/* ① 咨询工作台（staffmessages） */}
      {view === "desk" && deskErr !== null && (
        <ErrorPanel error={deskErr} onRetry={load} />
      )}
      {view === "desk" && deskErr === null && (
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
      {view === "ticket" && ticketErr !== null && (
        <ErrorPanel error={ticketErr} onRetry={loadTickets} />
      )}
      {view === "ticket" && ticketErr === null && (
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
      <ConfirmDialog
        open={askBulk !== null}
        onClose={() => setAskBulk(null)}
        onConfirm={() => {
          const a = askBulk;
          setAskBulk(null);
          if (a) void bulkDesk(a);
        }}
        tone={askBulk === "delete" ? "danger" : "neutral"}
        title={dict.review.confirmBulkDeleteTitle}
        message={fmt(dict.review.confirmBulkDeleteBody, { n: deskSel.length })}
        busy={busy}
      />
    </section>
  );
}
