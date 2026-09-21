"use client";

import { useCallback, useEffect, useState } from "react";
import { api, ApiError } from "@/lib/api-client";
import { useI18n } from "@/i18n/client";
import {
  MessageTable,
  type MessageRow,
  type PmBox,
} from "@/components/message-center-table";
import { ComposeForm, BoxManager } from "@/components/message-center-compose";
import { ToolbarSwitcher, BulkBar } from "@/components/message-center-toolbar";

// 站内消息中心（messages.php 全功能口径）：
// 收件箱/发件箱/自建文件夹 · 搜索 · 未读筛选 · 批量已读/删除/移动 ·
// 回复（Re: + 引用原文）/ 转发 · 文件夹管理（建/改名/删=清空名）。
// 拆出：契约/列表表格 @/components/message-center-table、
// 写信道/文件夹管理 @/components/message-center-compose、
// 切换工具条/批量操作条 @/components/message-center-toolbar。

export function MessageCenter() {
  const { dict } = useI18n();
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
          e instanceof ApiError
            ? (dict.errors[e.code] ?? e.message)
            : dict.common.loadFailed,
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
        e2 instanceof ApiError
          ? (dict.errors[e2.code] ?? e2.message)
          : dict.common.networkError,
      );
    } finally {
      setBusy(false);
    }
  }

  async function bulk(
    action: "markread" | "delete" | "move",
    target?: number | null,
  ) {
    if (selected.length === 0) return;
    setBusy(true);
    try {
      if (action === "markread") {
        await api.post("/api/v1/messages/markread", { ids: selected });
      } else if (action === "delete") {
        await api.post("/api/v1/messages/delete", { ids: selected });
      } else {
        await api.post("/api/v1/messages/move", {
          ids: selected,
          folder: target ?? null,
        });
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

  return (
    <section className="flex flex-col gap-3">
      {/* 切换 + 写信 + 搜索/筛选 */}
      <ToolbarSwitcher
        t={t}
        box={box}
        folder={folder}
        boxes={boxes}
        search={search}
        setSearch={setSearch}
        onlyUnread={onlyUnread}
        setOnlyUnread={setOnlyUnread}
        onShowInbox={() => {
          setBox("inbox");
          setFolder(null);
          setOpenId(null);
        }}
        onShowFolder={(id) => {
          setBox("inbox");
          setFolder(id);
          setOpenId(null);
        }}
        onShowSent={() => {
          setBox("sent");
          setOpenId(null);
        }}
        onToggleManage={() => setBoxManage((v) => !v)}
        onCompose={() => {
          setReplyTo(null);
          setForwardOf(null);
          setTo("");
          setSubject("");
          setBody("");
          setComposing((v) => !v);
        }}
      />

      {/* 文件夹管理（建/改名/删除=清空名） */}
      {boxManage && (
        <BoxManager
          t={t}
          boxes={boxes}
          newBoxName={newBoxName}
          setNewBoxName={setNewBoxName}
          busy={busy}
          onUpsert={(id, name) => void upsertBox(id, name)}
        />
      )}

      {composing && (
        <ComposeForm
          t={t}
          replyTo={replyTo}
          forwardOf={forwardOf}
          to={to}
          setTo={setTo}
          subject={subject}
          setSubject={setSubject}
          body={body}
          setBody={setBody}
          busy={busy}
          msg={msg}
          onSend={send}
          onCancel={() => setComposing(false)}
        />
      )}

      {/* 批量操作条 */}
      {rows && rows.length > 0 && (
        <BulkBar
          t={t}
          box={box}
          boxes={boxes}
          selected={selected}
          allSelected={selected.length === rows.length}
          onToggleAll={(on) => setSelected(on ? rows.map((r) => r.id) : [])}
          busy={busy}
          onMarkRead={() => void bulk("markread")}
          onMove={(f) => void bulk("move", f)}
          onDelete={() => void bulk("delete")}
        />
      )}

      {err && <p className="py-4 text-center text-sm text-sub">{err}</p>}
      {!err && rows === null && (
        <p className="py-4 text-center text-sm text-sub">{t.loading}</p>
      )}
      {rows && rows.length === 0 && (
        <p className="py-4 text-center text-sm text-sub">{t.empty}</p>
      )}
      {rows && rows.length > 0 && (
        <MessageTable
          box={box}
          rows={rows}
          selected={selected}
          onToggle={(id, on) =>
            setSelected((s) => (on ? [...s, id] : s.filter((x) => x !== id)))
          }
          openId={openId}
          onOpen={openMsg}
          onReply={startReply}
          onForward={startForward}
        />
      )}
    </section>
  );
}
