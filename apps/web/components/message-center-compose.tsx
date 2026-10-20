"use client";

import { useState } from "react";
import { useI18n } from "@/i18n/client";

/** 消息中心写信道与文件夹管理（从 components/message-center.tsx 按域拆出）：
 *  ComposeForm 回复/转发/新写三态表单；BoxManager 建/改名/删除（=清空名）。
 *  状态与发送逻辑留在 MessageCenter，动作回调注入。
 *  反馈协议（W-10）：发送/批量/文件夹操作的结果统一走 toast（见
 *  MessageCenter），本组件不再渲染行内 msg 文字。
 *  改名/删除二次交互（P2 清偿）：window.prompt/confirm → ConfirmDialog
 *  （改名用 children 输入槽，删除走 danger）。 */

import { BusyLabel } from "@/components/ui/busy-label";
import { ConfirmDialog } from "@/components/ui/confirm-dialog";
import { inputCls } from "@/components/message-center-table";
import { Icon } from "@/components/icons";

export function ComposeForm({
  t,
  replyTo,
  forwardOf,
  to,
  setTo,
  subject,
  setSubject,
  body,
  setBody,
  busy,
  onSend,
  onCancel,
}: {
  t: ReturnType<typeof useI18n>["dict"]["messages"];
  replyTo: number | null;
  forwardOf: number | null;
  to: string;
  setTo: (v: string) => void;
  subject: string;
  setSubject: (v: string) => void;
  body: string;
  setBody: (v: string) => void;
  busy: boolean;
  onSend: (e: React.FormEvent) => void;
  onCancel: () => void;
}) {
  return (
    <form
      onSubmit={onSend}
      className="nexus-table flex flex-col gap-2 !border-0 p-0"
    >
      <thead>
        <tr>
          <td className="colhead">
            {forwardOf
              ? t.forwardTitle
              : replyTo
                ? t.replyTitle
                : t.composeTitle}
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
              className="min-h-[100px] rounded-[var(--r-sm)] border border-[var(--brand-line)] bg-[var(--brand-paper)] px-3 py-2 text-sm text-ink outline-none focus:border-[var(--brand-orange)]"
            />
            {replyTo && <p className="text-[11px] text-sub">{t.quoteHint}</p>}
            <div className="flex items-center gap-3">
              {/* W-10：busy 态走 BusyLabel 原语（spinner 与文案同色同行，
                  替代裸文字切换），提交语义仍由本表单掌握 */}
              <button
                type="submit"
                disabled={busy}
                aria-busy={busy || undefined}
                className="min-h-[44px] rounded-[10px] border border-[var(--brand-orange-dark)] bg-[linear-gradient(135deg,var(--brand-orange-bright),var(--brand-orange))] px-5 text-sm font-bold text-white active:scale-[0.97] disabled:opacity-50"
              >
                <BusyLabel label={t.send} busy={busy} size="sm" />
              </button>
              <button
                type="button"
                onClick={onCancel}
                className="mainmenu-link min-h-[44px]"
              >
                {t.backToList}
              </button>
            </div>
          </td>
        </tr>
      </tbody>
    </form>
  );
}

export function BoxManager({
  t,
  boxes,
  newBoxName,
  setNewBoxName,
  busy,
  onUpsert,
}: {
  t: ReturnType<typeof useI18n>["dict"]["messages"];
  boxes: { id: number; name: string; count: number }[];
  newBoxName: string;
  setNewBoxName: (v: string) => void;
  busy: boolean;
  onUpsert: (id: number | null, name: string) => void;
}) {
  // 共用一个对话框状态机：rename=输入槽改名，delete=danger 删除确认
  const [ask, setAsk] = useState<
    | { kind: "rename"; id: number; name: string }
    | { kind: "delete"; id: number; name: string }
    | null
  >(null);
  const [renameTo, setRenameTo] = useState("");

  return (
    <div className="brand-panel flex flex-wrap items-center gap-2 p-3">
      <input
        value={newBoxName}
        onChange={(e) => setNewBoxName(e.target.value)}
        placeholder={t.newBoxPh}
        maxLength={14}
        className="min-h-[44px] w-44 rounded-[var(--r-sm)] border border-[var(--brand-line)] bg-[var(--brand-paper)] px-3 text-sm"
      />
      <button
        type="button"
        disabled={busy || !newBoxName.trim()}
        onClick={() => onUpsert(null, newBoxName.trim())}
        className="min-h-[44px] rounded-full bg-sky-deep px-4 text-xs font-bold text-white disabled:opacity-50"
      >
        {t.addBox}
      </button>
      {boxes.map((b) => (
        <span key={b.id} className="flex items-center gap-1">
          <button
            type="button"
            onClick={() => {
              setRenameTo(b.name);
              setAsk({ kind: "rename", id: b.id, name: b.name });
            }}
            aria-label={`${t.renameBoxPh}: ${b.name}`}
            title={t.renameBoxPh}
            className="inline-flex min-h-[36px] items-center gap-1 rounded-full
              border border-[var(--brand-line)] px-3 text-xs"
          >
            <Icon name="edit" size={13} />
            {b.name}
          </button>
          <button
            type="button"
            onClick={() => setAsk({ kind: "delete", id: b.id, name: b.name })}
            aria-label={`${t.deleteBox}: ${b.name}`}
            title={t.deleteBox}
            className="inline-flex min-h-[36px] items-center rounded-full border
              border-[var(--danger-border)] px-2 text-danger"
          >
            <Icon name="close" size={14} />
          </button>
        </span>
      ))}
      <ConfirmDialog
        open={ask !== null}
        onClose={() => setAsk(null)}
        onConfirm={() => {
          if (ask?.kind === "rename" && renameTo.trim()) {
            onUpsert(ask.id, renameTo.trim());
          } else if (ask?.kind === "delete") {
            onUpsert(ask.id, "");
          }
          setAsk(null);
        }}
        tone={ask?.kind === "delete" ? "danger" : "neutral"}
        title={ask?.kind === "delete" ? t.deleteBoxConfirm : t.renameBoxPh}
        busy={busy}
      >
        {ask?.kind === "rename" ? (
          <label className="flex flex-col gap-1 text-sm">
            <span>{t.renameBoxPh}</span>
            <input
              value={renameTo}
              onChange={(e) => setRenameTo(e.target.value)}
              maxLength={14}
              className="min-h-[40px] rounded-[var(--r-sm)] border
                border-[var(--brand-line)] bg-[var(--brand-paper)] px-3 text-sm"
              data-autofocus
            />
          </label>
        ) : null}
      </ConfirmDialog>
    </div>
  );
}
