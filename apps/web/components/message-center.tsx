"use client";

import { useCallback, useEffect, useState } from "react";
import { api, ApiError } from "@/lib/api-client";
import { useI18n, apiErrorMessage } from "@/i18n/client";
import { toast } from "@/components/ui/toast";
import {
  MessageTable,
  type MessageRow,
  type PmBox,
} from "@/components/message-center-table";
import { ComposeForm, BoxManager } from "@/components/message-center-compose";
import { ToolbarSwitcher, BulkBar } from "@/components/message-center-toolbar";
import { StatePanel } from "@/components/ui/state-panel";
import { SkeletonRows } from "@/components/ui/skeleton";
import { ConfirmDialog } from "@/components/ui/confirm-dialog";

// 站内消息中心（messages.php 全功能口径）：
// 收件箱/发件箱/自建文件夹 · 搜索 · 未读筛选 · 批量已读/删除/移动 ·
// 回复（Re: + 引用原文）/ 转发 · 文件夹管理（建/改名/删=清空名）。
// 拆出：表格 @/components/message-center-table、写信/文件夹
// @/components/message-center-compose、工具条 @/…-toolbar。
// 反馈协议（W-10）：写操作统一 toast；批量删除二次确认（W-23）
// 走 ConfirmDialog danger，window.confirm 退役。

export function MessageCenter() {
  const { dict } = useI18n();
  // 深链参数一次读齐：userbar 的 ?box=sent、通知中心的 ?unread=true
  const params = typeof window === "undefined"
    ? null
    : new URLSearchParams(window.location.search);
  const [box, setBox] = useState<"inbox" | "sent">(
    () => (params?.get("box") === "sent" ? "sent" : "inbox"));
  const [folder, setFolder] = useState<number | null>(null); // null=主收件箱
  const [rows, setRows] = useState<MessageRow[] | null>(null);
  const [boxes, setBoxes] = useState<PmBox[]>([]);
  const [selected, setSelected] = useState<number[]>([]);
  const [search, setSearch] = useState("");
  const [onlyUnread, setOnlyUnread] = useState(
    () => params?.get("unread") === "true");
  const [err, setErr] = useState<string | null>(null);
  const [composing, setComposing] = useState(false);
  const [replyTo, setReplyTo] = useState<number | null>(null);
  const [forwardOf, setForwardOf] = useState<number | null>(null);
  const [to, setTo] = useState("");
  const [subject, setSubject] = useState("");
  const [body, setBody] = useState("");
  const [busy, setBusy] = useState(false);
  const [openId, setOpenId] = useState<number | null>(null);
  const [boxManage, setBoxManage] = useState(false);
  const [newBoxName, setNewBoxName] = useState("");
  // 批量删除二次确认（W-23：window.confirm → ConfirmDialog danger）
  const [askBulkDelete, setAskBulkDelete] = useState(false);
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
      .catch((e) => setErr(apiErrorMessage(dict, e) || dict.common.loadFailed));
    api
      .get<PmBox[]>("/api/v1/messages/boxes")
      .then(setBoxes)
      .catch(() => { setBoxes([]); toast.error(dict.common.loadFailed); });
  }, [box, folder, search, onlyUnread, dict]);

  useEffect(() => {
    load();
  }, [load]);

  // 打开详情即置已读（短路同条重复打开）
  function openMsg(m: MessageRow) {
    if (openId === m.id) return setOpenId(null);
    setOpenId(m.id);
    if (box !== "inbox" || !m.unread) return;
    api
      .post("/api/v1/messages/markread", { ids: [m.id] })
      .then(() => load())
      .catch(() => {});
  }

  // 回复/转发共用落地（re:/fw: 前缀不重复叠加）
  function startCompose(m: MessageRow, mode: "reply" | "forward") {
    setReplyTo(mode === "reply" ? m.id : null);
    setForwardOf(mode === "forward" ? m.id : null);
    setTo(mode === "reply" ? (m.counterpart ?? "") : "");
    const pfx = mode === "reply" ? "Re" : "Fw";
    setSubject(
      new RegExp(`^${pfx}:`, "i").test(m.subject)
        ? m.subject
        : `${pfx}: ${m.subject}`,
    );
    setBody("");
    setComposing(true);
    setOpenId(null);
  }

  async function send(e: React.FormEvent) {
    e.preventDefault();
    setBusy(true);
    try {
      await api.post("/api/v1/messages", {
        to: to.trim(),
        subject: subject.trim(),
        body: body.trim(),
        ...(replyTo ? { reply_to: replyTo } : {}),
        ...(forwardOf ? { forward_of: forwardOf } : {}),
      });
      toast.success(t.sentOk);
      setTo("");
      setSubject("");
      setBody("");
      setReplyTo(null);
      setForwardOf(null);
      setComposing(false);
      setBox("sent");
    } catch (e2) {
      toastError(e2);
    } finally {
      setBusy(false);
    }
  }

  /** 写操作失败统一出口：字典文案优先 + request_id 芯片 */
  function toastError(e: unknown) {
    const ae = e instanceof ApiError ? e : undefined;
    toast.error(
      ae ? (apiErrorMessage(dict, ae)) : dict.common.networkError,
      { requestId: ae?.requestId },
    );
  }

  async function bulk(
    action: "markread" | "delete" | "move",
    target?: number | null,
  ) {
    if (selected.length === 0) return;
    // ZT81：删除二次确认在渲染区 ConfirmDialog（W-23），此处只执行
    setBusy(true);
    const path =
      `/api/v1/messages/${action === "markread" ? "markread" : action}`;
    try {
      await api.post(path, {
        ids: selected,
        ...(action === "move" ? { folder: target ?? null } : {}),
      });
      load();
      toast.success(t.bulkDone.replace("{n}", String(selected.length)));
    } catch (e) {
      toastError(e);
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
      toast.success(id ? t.boxRenamed : t.boxCreated);
    } catch (e) {
      toastError(e);
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
          onDelete={() => setAskBulkDelete(true)}
        />
      )}

      {/* 批量删除二次确认（W-16/W-23：danger 口径，遮罩不可点关） */}
      <ConfirmDialog
        open={askBulkDelete}
        onClose={() => setAskBulkDelete(false)}
        onConfirm={() => {
          setAskBulkDelete(false);
          void bulk("delete");
        }}
        tone="danger"
        title={t.deleteSelTitle}
        message={t.deleteSelConfirm.replace("{n}", String(selected.length))}
        busy={busy}
      />
      {/* 四态路由（W-03 试点）：StatePanel 调度四态 */}
      <StatePanel
        loading={!err && rows === null}
        error={err}
        empty={!err && rows !== null && rows.length === 0}
        emptyTitle={t.empty}
        onRetry={load}
        loadingFallback={<SkeletonRows rows={8} />}
      >
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
            onReply={(m) => startCompose(m, "reply")}
            onForward={(m) => startCompose(m, "forward")}
          />
        )}
      </StatePanel>
    </section>
  );
}
