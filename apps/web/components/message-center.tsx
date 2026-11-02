"use client";

import { useCallback, useEffect, useRef, useState } from "react";
import { api, ApiError, pagedGet } from "@/lib/api-client";
import { useI18n, apiErrorMessage } from "@/i18n/client";
import { toast } from "@/components/ui/toast";
import {
  MessageTable,
  type MessageRow,
  type PmBox,
} from "@/components/message-center-table";
import { MessageCards } from "@/components/message-center-cards";
import { ComposeForm, BoxManager } from "@/components/message-center-compose";
import { useMessageCompose } from "@/components/message-center-compose-hook";
import { useMessageActions } from "@/components/message-center-actions";
import { ToolbarSwitcher, BulkBar } from "@/components/message-center-toolbar";
import { StatePanel } from "@/components/ui/state-panel";
import { SkeletonRows } from "@/components/ui/skeleton";
import { ConfirmDialog } from "@/components/ui/confirm-dialog";
import { Pager } from "@/components/ui/pager";
import { useIsCompact } from "@/lib/hooks/use-media";

// 站内消息中心（messages.php 全功能口径）：
// 收件箱/发件箱/自建文件夹 · 搜索 · 未读筛选 · 批量已读/删除/移动 ·
// 回复（Re: + 引用原文）/ 转发 · 文件夹管理（建/改名/删=清空名）。
// 拆出：表格 @/components/message-center-table、手机卡片
// @/components/message-center-cards、写信/文件夹 @/…-compose、
// 写信状态机 @/…-compose-hook、工具条 @/…-toolbar、
// 系统通知取词 @/…-notice。
// 反馈协议（W-10）：写操作统一 toast；批量删除二次确认（W-23）
// 走 ConfirmDialog danger，window.confirm 退役。

/** 每页条数：窄屏是卡片，20 条正好一屏多；桌面保持原样（一次 100 条）。
 *  接口早已支持 limit/offset（总数走 X-Total-Count 头），此前前端没用过
 *  ——窄屏一次拉 100 条，root 的 121 封只看得到 100 封，角标与列表对不上。
 *  桌面不分页：它一次 100 行是既有行为（桌面表格本来就一屏放得下），
 *  改了就动到没被投诉的页面。 */
const PAGE_MOBILE = 20;
const PAGE_DESKTOP = 100;

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
  const [page, setPage] = useState(1);
  const [total, setTotal] = useState<number | null>(null);
  const [openId, setOpenId] = useState<number | null>(null);
  const [boxManage, setBoxManage] = useState(false);
  const [newBoxName, setNewBoxName] = useState("");
  // 批量删除二次确认（W-23：window.confirm → ConfirmDialog danger）
  const [askBulkDelete, setAskBulkDelete] = useState(false);
  // 窄屏「⋯」面板（只看未读 + 批量操作）
  const [moreOpen, setMoreOpen] = useState(false);
  const t = dict.messages;

  const compact = useIsCompact();
  const pageSize = compact ? PAGE_MOBILE : PAGE_DESKTOP;
  // 并发守卫：窄屏（20/页）与桌面（100/页）两条请求会重叠，慢响应后到
  // 会把窄屏那页覆盖成 100 条（页高翻三倍）。用序号只认最后一次。
  const listSeqRef = useRef(0);
  const load = useCallback(() => {
    const seq = ++listSeqRef.current;
    setRows(null);
    setErr(null);
    setSelected([]);
    const qs = new URLSearchParams();
    if (folder !== null && box === "inbox") qs.set("box_id", String(folder));
    if (search.trim()) qs.set("search", search.trim());
    if (onlyUnread && box === "inbox") qs.set("unread", "true");
    qs.set("limit", String(pageSize));
    qs.set("offset", String((page - 1) * pageSize));
    pagedGet<MessageRow>(`/api/v1/messages/${box}?${qs.toString()}`)
      .then((r) => {
        if (seq !== listSeqRef.current) return;
        setRows(r.items);
        setTotal(r.total);
      })
      .catch((e) => {
        if (seq !== listSeqRef.current) return;
        setErr(apiErrorMessage(dict, e) || dict.common.loadFailed);
      });
    api
      .get<PmBox[]>("/api/v1/messages/boxes")
      .then(setBoxes)
      .catch(() => { setBoxes([]); toast.error(dict.common.loadFailed); });
  }, [box, folder, search, onlyUnread, page, pageSize, dict]);

  useEffect(() => {
    load();
  }, [load]);

  const act = useMessageActions({
    load,
    selected,
    setAskBulkDelete,
    onBoxSaved: () => {
      setNewBoxName("");
      setBoxManage(false);
    },
  });

  const compose = useMessageCompose(() => {
    setBox("sent");
    setPage(1);
    load();
  });

  /** 打开详情即置已读（短路同条重复打开） */
  function openMsg(m: MessageRow) {
    if (openId === m.id) return setOpenId(null);
    setOpenId(m.id);
    if (box !== "inbox" || !m.unread) return;
    api
      .post("/api/v1/messages/markread", { ids: [m.id] })
      .then(() => load())
      .catch((e) => toast.error(apiErrorMessage(dict, e)));
  }

  const pages = total === null ? 1 : Math.max(1, Math.ceil(total / pageSize));
  const bulkBar =
    rows && rows.length > 0 ? (
      <BulkBar
        t={t}
        box={box}
        boxes={boxes}
        selected={selected}
        allSelected={selected.length === rows.length}
        onToggleAll={(on) =>
          setSelected(on ? rows.map((r) => r.id) : [])
        }
        busy={act.busy}
        onMarkRead={() => void act.bulk("markread")}
        onMove={(f) => void act.bulk("move", f)}
        onDelete={() => setAskBulkDelete(true)}
      />
    ) : null;
  /** 回复/转发：先收起已展开的详情，再进写信态 */
  const startReply = (m: MessageRow, mode: "reply" | "forward") => {
    setOpenId(null);
    compose.startFrom(m, mode);
  };
  /** 换信箱/文件夹/筛选 ⇒ 回到第一页（否则会停在一个不存在的页码上） */
  const backToFirst = () => setPage(1);

  return (
    <section className="flex flex-col gap-3">
      {/* 切换 + 写信 + 搜索/筛选 */}
      <ToolbarSwitcher
        t={t}
        box={box}
        folder={folder}
        boxes={boxes}
        search={search}
        setSearch={(v) => { setSearch(v); backToFirst(); }}
        onlyUnread={onlyUnread}
        setOnlyUnread={(v) => { setOnlyUnread(v); backToFirst(); }}
        onShowInbox={() => {
          setBox("inbox");
          setFolder(null);
          setOpenId(null);
          backToFirst();
        }}
        onShowFolder={(id) => {
          setBox("inbox");
          setFolder(id);
          setOpenId(null);
          backToFirst();
        }}
        onShowSent={() => {
          setBox("sent");
          setOpenId(null);
          backToFirst();
        }}
        onToggleManage={() => setBoxManage((v) => !v)}
        managing={boxManage}
        onCompose={compose.open}
        moreOpen={moreOpen}
        onToggleMore={() => setMoreOpen((v) => !v)}
        bulk={bulkBar}
      />
      {/* 文件夹管理（建/改名/删除=清空名） */}
      {boxManage && (
        <BoxManager
          t={t}
          boxes={boxes}
          newBoxName={newBoxName}
          setNewBoxName={setNewBoxName}
          busy={act.busy}
          onUpsert={(id, name) => void act.upsertBox(id, name)}
        />
      )}

      {compose.composing && (
        <ComposeForm
          t={t}
          replyTo={compose.replyTo}
          forwardOf={compose.forwardOf}
          to={compose.to}
          setTo={compose.setTo}
          subject={compose.subject}
          setSubject={compose.setSubject}
          body={compose.body}
          setBody={compose.setBody}
          busy={compose.busy}
          onSend={compose.send}
          onCancel={() => compose.setComposing(false)}
        />
      )}

      {/* 批量操作条：桌面常驻，窄屏进工具条的「⋯」面板（同一份内容） */}
      <div className={moreOpen ? "block sm:hidden" : "hidden sm:block"}>
        {bulkBar}
      </div>

      {/* 批量删除二次确认（W-16/W-23：danger 口径，遮罩不可点关） */}
      <ConfirmDialog
        open={askBulkDelete}
        onClose={() => setAskBulkDelete(false)}
        onConfirm={() => {
          setAskBulkDelete(false);
          void act.bulk("delete");
        }}
        tone="danger"
        title={t.deleteSelTitle}
        message={t.deleteSelConfirm.replace("{n}", String(selected.length))}
        busy={act.busy}
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
          <>
            {/* 桌面：六列表格 */}
            <div className="hidden sm:block">
              <MessageTable
                box={box}
                rows={rows}
                selected={selected}
                onToggle={(id, on) =>
                  setSelected((s) =>
                    on ? [...s, id] : s.filter((x) => x !== id),
                  )
                }
                openId={openId}
                onOpen={openMsg}
                onReply={(m) => startReply(m, "reply")}
                onForward={(m) => startReply(m, "forward")}
              />
            </div>
            {/* 手机：卡片（发件人/时间回到第一层） */}
            <MessageCards
              box={box}
              rows={rows}
              openId={openId}
              onOpen={openMsg}
              onReply={(m) => startReply(m, "reply")}
              onForward={(m) => startReply(m, "forward")}
            />
            {compact && (
              <Pager
                page={page}
                pages={pages}
                total={total ?? undefined}
                onStep={(p) => {
                  setOpenId(null);
                  setPage(p);
                }}
              />
            )}
          </>
        )}
      </StatePanel>
    </section>
  );
}
