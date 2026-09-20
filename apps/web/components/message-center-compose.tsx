"use client";

import { useI18n } from "@/i18n/client";

/** 消息中心写信道与文件夹管理（从 components/message-center.tsx 按域拆出）：
 *  ComposeForm 回复/转发/新写三态表单；BoxManager 建/改名/删除（=清空名）。
 *  状态与发送逻辑留在 MessageCenter，动作回调注入。 */

import { inputCls } from "@/components/message-center-table";

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
  msg,
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
  msg: string | null;
  onSend: (e: React.FormEvent) => void;
  onCancel: () => void;
}) {
  return (
    <form onSubmit={onSend} className="nexus-table flex flex-col gap-2 !border-0 p-0">
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
                onClick={onCancel}
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
  return (
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
              const n = window.prompt(t.renameBoxPh, b.name);
              if (n !== null && n.trim()) onUpsert(b.id, n.trim());
            }}
            className="min-h-[36px] rounded-full border border-[var(--baozi-line)] px-3 text-xs"
          >
            ✎ {b.name}
          </button>
          <button
            type="button"
            onClick={() => {
              if (window.confirm(t.deleteBoxConfirm)) onUpsert(b.id, "");
            }}
            className="min-h-[36px] rounded-full px-2 text-xs text-danger"
          >
            ✕
          </button>
        </span>
      ))}
    </div>
  );
}
