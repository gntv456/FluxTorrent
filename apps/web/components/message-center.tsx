"use client";

import { useCallback, useEffect, useState } from "react";
import { api, ApiError } from "@/lib/api-client";
import { useI18n } from "@/i18n/client";
import { dateLocale } from "@/i18n/config";
import { MarkdownRenderer } from "@/components/forum-markdown";

interface MessageRow {
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

interface PmBox {
  id: number;
  name: string;
  count: number;
}

/** 站内消息中心（messages.php 全功能口径）：
 *  收件箱/发件箱/自建文件夹 · 搜索 · 未读筛选 · 批量已读/删除/移动 ·
 *  回复（Re: + 引用原文）/ 转发 · 文件夹管理（建/改名/删=清空名）。 */
export function MessageCenter() {
  const { dict, locale } = useI18n();
  // 支持 userbar 深链：/messages?box=sent
  const [box, setBox] = useState<"inbox" | "sent">(() => {
    if (typeof window === "undefined") return "inbox";
    return new URLSearchParams(window.location.search).get("box") === "sent"
      ? "sent"
      : "inbox";
  });
  const [folder, setFolder] = useState<number | null>(null); // null=主收件箱
  const [rows, setRows] = useState<MessageRow[] | null>(null);
  const [boxes, setBoxes] = useState<PmBox[]>([]);
  const [selected, setSelected] = useState<number[]>([]);
  const [search, setSearch] = useState("");
  const [onlyUnread, setOnlyUnread] = useState(false);
  const [err, setErr] = useState<string | null>(null);
  const [composing, setComposing] = useState(false);
  const [replyTo, setReplyTo] = useState<number | null>(null);
  const [forwardOf, setForwardOf] = useState<number | null>(null);
  const [to, setTo] = useState("");
  const [subject, setSubject] = useState("");
  const [body, setBody] = useState("");
  const [msg, setMsg] = useState<string | null>(null);
  const [busy, setBusy] = useState(false);
  const [openId, setOpenId] = useState<number | null>(null);
  const [boxManage, setBoxManage] = useState(false);
  const [newBoxName, setNewBoxName] = useState("");
  const t = dict.messages;

  const load = useCallback(() => {
    setRows(null);
    setErr(null);
    setSelected([]);
    const qs = new URLSearchParams();
    if (folder !== null && box === "inbox") qs.set("box_id", String(folder));
    if (search.trim()) qs.set("search", search.trim());
    if (onlyUnread && box === "inbox") qs.set("unread", "true");
    const suffix = qs.toString() ? `?${qs}` : "";
    api
      .get<MessageRow[]>(`/api/v1/messages/${box}${suffix}`)
      .then(setRows)
      .catch((e) =>
        setErr(
          e instanceof ApiError ? (dict.errors[e.code] ?? e.message) : dict.common.loadFailed,
        ),
      );
    api
      .get<PmBox[]>("/api/v1/messages/boxes")
      .then(setBoxes)
      .catch(() => setBoxes([]));
  }, [box, folder, search, onlyUnread, dict]);

  useEffect(() => {
    load();
  }, [load]);

  // 打开详情即置已读
  function openMsg(m: MessageRow) {
    if (openId === m.id) {
      setOpenId(null);
      return;
    }
    setOpenId(m.id);
    if (box === "inbox" && m.unread) {
      api
        .post("/api/v1/messages/markread", { ids: [m.id] })
        .then(() => load())
        .catch(() => {});
    }
  }

  function startReply(m: MessageRow) {
    setReplyTo(m.id);
    setForwardOf(null);
    setTo(m.counterpart ?? "");
    setSubject(/^re:/i.test(m.subject) ? m.subject : `Re: ${m.subject}`);
    setBody("");
    setComposing(true);
    setOpenId(null);
  }

  function startForward(m: MessageRow) {
    setForwardOf(m.id);
    setReplyTo(null);
    setTo("");
    setSubject(/^fw:/i.test(m.subject) ? m.subject : `Fw: ${m.subject}`);
    setBody("");
    setComposing(true);
    setOpenId(null);
  }

  async function send(e: React.FormEvent) {
    e.preventDefault();
    setBusy(true);
    setMsg(null);
    try {
      await api.post("/api/v1/messages", {
        to: to.trim(),
        subject: subject.trim(),
        body: body.trim(),
        ...(replyTo ? { reply_to: replyTo } : {}),
        ...(forwardOf ? { forward_of: forwardOf } : {}),
      });
      setMsg(t.sentOk);
      setTo("");
      setSubject("");
      setBody("");
      setReplyTo(null);
      setForwardOf(null);
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

  async function bulk(action: "markread" | "delete" | "move", target?: number | null) {
    if (selected.length === 0) return;
    setBusy(true);
    try {
      if (action === "markread") {
        await api.post("/api/v1/messages/markread", { ids: selected });
      } else if (action === "delete") {
        await api.post("/api/v1/messages/delete", { ids: selected });
      } else {
        await api.post("/api/v1/messages/move", { ids: selected, folder: target ?? null });
      }
      load();
    } catch (e) {
      setMsg(e instanceof ApiError ? e.message : dict.common.networkError);
    } finally {
      setBusy(false);
    }
  }

  async function upsertBox(id: number | null, name: string) {
    setBusy(true);
    try {
      await api.post("/api/v1/messages/boxes", { id, name });
      setNewBoxName("");
      setBoxManage(false);
      load();
    } catch (e) {
      setMsg(e instanceof ApiError ? e.message : dict.common.networkError);
    } finally {
      setBusy(false);
    }
  }

  const inputCls =
    "min-h-[44px] w-full rounded-[var(--r-sm)] border border-[var(--baozi-line)] bg-[var(--baozi-paper)] px-3 text-sm text-ink outline-none focus:border-[var(--baozi-orange)]";

  return (
    <section className="flex flex-col gap-3">
      {/* 切换 + 写信 + 搜索/筛选 */}
      <div className="flex flex-wrap items-center justify-between gap-2">
        <div className="flex flex-wrap gap-2">
          <button
            type="button"
            onClick={() => {
              setBox("inbox");
              setFolder(null);
              setOpenId(null);
            }}
            aria-current={box === "inbox" ? "true" : undefined}
            className={`min-h-[44px] rounded-[10px] border px-4 text-sm font-bold ${
              box === "inbox"
                ? "border-[var(--baozi-orange)] bg-[linear-gradient(135deg,var(--baozi-orange-bright),var(--baozi-orange))] text-white shadow-[0_5px_13px_var(--accent-shadow)]"
                : "border-[var(--baozi-line)] bg-[var(--row-veil-soft)] text-ink hover:-translate-y-px hover:text-[var(--baozi-orange)]"
            }`}
          >
            {t.inbox}
          </button>
          {boxes.map((b) => (
            <button
              key={b.id}
              type="button"
              onClick={() => {
                setBox("inbox");
                setFolder(b.id);
                setOpenId(null);
              }}
              aria-current={folder === b.id ? "true" : undefined}
              className={`min-h-[44px] rounded-[10px] border px-4 text-sm font-bold ${
                folder === b.id
                  ? "border-[var(--baozi-orange)] bg-[var(--sky-soft)] text-[var(--baozi-orange-dark)]"
                  : "border-[var(--baozi-line)] bg-[var(--row-veil-soft)] text-ink"
              }`}
            >
              {b.name} ({b.count})
            </button>
          ))}
          <button
            type="button"
            onClick={() => {
              setBox("sent");
              setOpenId(null);
            }}
            aria-current={box === "sent" ? "true" : undefined}
            className={`min-h-[44px] rounded-[10px] border px-4 text-sm font-bold ${
              box === "sent"
                ? "border-[var(--baozi-orange)] bg-[linear-gradient(135deg,var(--baozi-orange-bright),var(--baozi-orange))] text-white"
                : "border-[var(--baozi-line)] bg-[var(--row-veil-soft)] text-ink"
            }`}
          >
            {t.sent}
          </button>
          <button
            type="button"
            onClick={() => setBoxManage((v) => !v)}
            className="min-h-[44px] rounded-[10px] border border-[var(--baozi-line)] bg-[var(--row-veil-soft)] px-3 text-sm text-sub"
            title={t.manageBoxes}
          >
            🗂
          </button>
        </div>
        <div className="flex flex-wrap items-center gap-2">
          {box === "inbox" && (
            <>
              <input
                value={search}
                onChange={(e) => setSearch(e.target.value)}
                placeholder={t.searchPh}
                className="min-h-[44px] w-40 rounded-[var(--r-sm)] border border-[var(--baozi-line)] bg-[var(--baozi-paper)] px-3 text-sm"
              />
              <label className="flex min-h-[44px] items-center gap-1 text-xs text-sub">
                <input
                  type="checkbox"
                  checked={onlyUnread}
                  onChange={(e) => setOnlyUnread(e.target.checked)}
                />
                {t.onlyUnread}
              </label>
            </>
          )}
          <button
            type="button"
            onClick={() => {
              setReplyTo(null);
              setForwardOf(null);
              setTo("");
              setSubject("");
              setBody("");
              setComposing((v) => !v);
            }}
            className="min-h-[44px] rounded-[10px] border border-[var(--baozi-orange-dark)] bg-[linear-gradient(135deg,var(--baozi-orange-bright),var(--baozi-orange))] px-4 text-sm font-bold text-white shadow-[var(--shadow-hover)] active:scale-[0.97]"
          >
            {t.compose}
          </button>
        </div>
      </div>

      {/* 文件夹管理（建/改名/删除=清空名） */}
      {boxManage && (
        <div className="baozi-panel flex flex-wrap items-center gap-2 p-3">
          <input
            value={newBoxName}
            onChange={(e) => setNewBoxName(e.target.value)}
            placeholder={t.newBoxPh}
            maxLength={14}
            className="min-h-[44px] w-44 rounded-[var(--r-sm)] border border-[var(--baozi-line)] bg-[var(--baozi-paper)] px-3 text-sm"
          />
          <button
            type="button"
            disabled={busy || !newBoxName.trim()}
            onClick={() => upsertBox(null, newBoxName.trim())}
            className="min-h-[44px] rounded-full bg-sky-deep px-4 text-xs font-bold text-white disabled:opacity-50"
          >
            {t.addBox}
          </button>
          {boxes.map((b) => (
            <span key={b.id} className="flex items-center gap-1">
              <button
                type="button"
                onClick={() => {
                  const n = window.prompt(t.renameBoxPh, b.name);
                  if (n !== null && n.trim()) void upsertBox(b.id, n.trim());
                }}
                className="min-h-[36px] rounded-full border border-[var(--baozi-line)] px-3 text-xs"
              >
                ✎ {b.name}
              </button>
              <button
                type="button"
                onClick={() => {
                  if (window.confirm(t.deleteBoxConfirm)) void upsertBox(b.id, "");
                }}
                className="min-h-[36px] rounded-full px-2 text-xs text-danger"
              >
                ✕
              </button>
            </span>
          ))}
        </div>
      )}

      {composing && (
        <form onSubmit={send} className="nexus-table flex flex-col gap-2 !border-0 p-0">
          <thead>
            <tr>
              <td className="colhead">
                {forwardOf ? t.forwardTitle : replyTo ? t.replyTitle : t.composeTitle}
              </td>
            </tr>
          </thead>
          <tbody>
            <tr>
              <td className="flex flex-col gap-2 p-3">
                <input
                  value={to}
                  onChange={(e) => setTo(e.target.value)}
                  placeholder={t.to}
                  required
                  maxLength={24}
                  className={inputCls}
                />
                <input
                  value={subject}
                  onChange={(e) => setSubject(e.target.value)}
                  placeholder={t.subject}
                  required
                  maxLength={120}
                  className={inputCls}
                />
                <textarea
                  value={body}
                  onChange={(e) => setBody(e.target.value)}
                  placeholder={replyTo ? t.replyBodyPh : t.body}
                  rows={4}
                  maxLength={5000}
                  className="min-h-[100px] rounded-[var(--r-sm)] border border-[var(--baozi-line)] bg-[var(--baozi-paper)] px-3 py-2 text-sm text-ink outline-none focus:border-[var(--baozi-orange)]"
                />
                {replyTo && <p className="text-[11px] text-sub">{t.quoteHint}</p>}
                <div className="flex items-center gap-3">
                  <button
                    type="submit"
                    disabled={busy}
                    className="min-h-[44px] rounded-[10px] border border-[var(--baozi-orange-dark)] bg-[linear-gradient(135deg,var(--baozi-orange-bright),var(--baozi-orange))] px-5 text-sm font-bold text-white active:scale-[0.97] disabled:opacity-50"
                  >
                    {busy ? t.sending : t.send}
                  </button>
                  <button
                    type="button"
                    onClick={() => setComposing(false)}
                    className="mainmenu-link min-h-[44px]"
                  >
                    {t.backToList}
                  </button>
                </div>
                {msg && (
                  <p role="status" className="text-sm text-sub">
                    {msg}
                  </p>
                )}
              </td>
            </tr>
          </tbody>
        </form>
      )}

      {/* 批量操作条 */}
      {rows && rows.length > 0 && (
        <div className="flex flex-wrap items-center gap-2 text-xs">
          <label className="flex items-center gap-1 text-sub">
            <input
              type="checkbox"
              checked={selected.length === rows.length}
              onChange={(e) => setSelected(e.target.checked ? rows.map((r) => r.id) : [])}
            />
            {t.selectAll}
          </label>
          {box === "inbox" && (
            <button
              type="button"
              disabled={selected.length === 0 || busy}
              onClick={() => bulk("markread")}
              className="min-h-[36px] rounded-full border border-[var(--baozi-line)] px-3 font-bold disabled:opacity-40"
            >
              ✓ {t.markRead}
            </button>
          )}
          {box === "inbox" && boxes.length > 0 && (
            <select
              disabled={selected.length === 0 || busy}
              onChange={(e) => {
                const v = e.target.value;
                void bulk("move", v === "" ? null : Number(v));
                e.target.value = "";
              }}
              className="min-h-[36px] rounded-full border border-[var(--baozi-line)] bg-[var(--baozi-paper)] px-3 font-bold disabled:opacity-40"
              defaultValue=""
            >
              <option value="">{t.moveTo}</option>
              <option value="">{t.moveToInbox}</option>
              {boxes.map((b) => (
                <option key={b.id} value={b.id}>
                  {t.moveToBox.replace("{name}", b.name)}
                </option>
              ))}
            </select>
          )}
          <button
            type="button"
            disabled={selected.length === 0 || busy}
            onClick={() => bulk("delete")}
            className="min-h-[36px] rounded-full border border-[var(--danger-border)] px-3 font-bold text-danger disabled:opacity-40"
          >
            🗑 {t.deleteSel}
          </button>
          <span className="text-sub">
            {t.selectedCount.replace("{n}", String(selected.length))}
          </span>
        </div>
      )}

      {err && <p className="py-4 text-center text-sm text-sub">{err}</p>}
      {!err && rows === null && (
        <p className="py-4 text-center text-sm text-sub">{t.loading}</p>
      )}
      {rows && rows.length === 0 && (
        <p className="py-4 text-center text-sm text-sub">{t.empty}</p>
      )}
      {rows && rows.length > 0 && (
        <table className="nexus-table">
          <thead>
            <tr>
              <th className="w-10">
                <span className="sr-only">{t.selectAll}</span>
              </th>
              <th className="w-16">{t.readCol}</th>
              <th>{t.subject}</th>
              <th className="hidden sm:table-cell">{box === "inbox" ? t.fromCol : t.toCol}</th>
              <th className="hidden w-44 md:table-cell">{t.timeCol}</th>
              <th className="w-16 text-right">{t.actionCol}</th>
            </tr>
          </thead>
          <tbody>
            {rows.map((m) => {
              const cp = m.counterpart ?? t.systemSender;
              const time = new Date(m.created_at).toLocaleString(dateLocale(locale));
              const unread = box === "inbox" && (m.unread ?? !m.read_at);
              const status = box === "inbox" ? (unread ? t.unreadTag : t.readTag) : t.sentTag;
              return (
                <tr key={m.id}>
                  <td>
                    <input
                      type="checkbox"
                      checked={selected.includes(m.id)}
                      onChange={(e) =>
                        setSelected((s) =>
                          e.target.checked ? [...s, m.id] : s.filter((x) => x !== m.id),
                        )
                      }
                    />
                  </td>
                  <td>
                    <span aria-hidden className="mr-1">
                      {box === "sent" ? "📤" : m.is_system ? "🔔" : unread ? "📬" : "📭"}
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
                      onClick={() => openMsg(m)}
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
                        <p className="mt-1 text-[11px] text-sub md:hidden">{cp} · {time}</p>
                        {/* 系统通知没有对端用户，回复/转发无意义（0123 视觉区分的一部分） */}
                        {!m.is_system && box === "inbox" && (
                          <div className="mt-2 flex justify-end gap-2">
                            <button
                              type="button"
                              onClick={() => startReply(m)}
                              className="min-h-[32px] rounded-full border border-[var(--baozi-orange-dark)] px-3 text-xs font-bold text-[var(--baozi-orange-dark)] hover:bg-[var(--baozi-orange)] hover:text-white"
                            >
                              ↩ {t.reply}
                            </button>
                            <button
                              type="button"
                              onClick={() => startForward(m)}
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
                  <td className="hidden text-[11px] text-sub md:table-cell">{time}</td>
                  <td className="text-right">
                    <button
                      type="button"
                      onClick={() => openMsg(m)}
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
      )}
    </section>
  );
}
