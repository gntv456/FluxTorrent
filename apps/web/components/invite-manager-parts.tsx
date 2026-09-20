"use client";

import { useI18n } from "@/i18n/client";
import { dateLocale } from "@/i18n/config";
import type { InviteItem } from "@/components/invite-manager";

/** 邀请管理展示件（从 invite-manager.tsx 按域拆出）：
 *  邀请码列表表格 + 发送邀请邮件弹层（数据装载与动作留在原文件）。 */

/** 邀请码列表：复制 / 状态角标 / 发送到邮箱 / 重发 / 过期展示 */
export function InviteList({
  invites,
  copiedId,
  busyId,
  onCopy,
  onSend,
}: {
  invites: InviteItem[];
  copiedId: number | null;
  busyId: number | null;
  onCopy: (inv: InviteItem) => void;
  onSend: (inv: InviteItem) => void;
}) {
  const { dict, locale } = useI18n();
  const t = dict.invites;
  const fmtDate = (s: string) =>
    new Date(s).toLocaleString(dateLocale(locale), {
      month: "2-digit",
      day: "2-digit",
      hour: "2-digit",
      minute: "2-digit",
    });
  return (
    <div className="baozi-wide-table-scroll">
      <table className="nexus-table">
        <tbody>
          <tr>
            <td className="colhead">{t.colCode}</td>
            <td className="colhead">{t.colStatus}</td>
            <td className="colhead">{t.colSentTo}</td>
            <td className="colhead">{t.colExpires}</td>
            <td className="colhead">{t.colAction}</td>
          </tr>
          {invites.map((i) => (
            <tr key={i.id}>
              <td>
                <button
                  type="button"
                  onClick={() => onCopy(i)}
                  className="num font-mono text-xs text-sky-deep hover:underline"
                  title={i.code}
                >
                  {copiedId === i.id ? t.copied : `${i.code.slice(0, 10)}••••`}
                </button>
              </td>
              <td>
                <span
                  className={`sticker ${
                    i.status === 0
                      ? "bg-sun text-ink"
                      : i.status === 1
                        ? "bg-mint/30 text-ink"
                        : "bg-cloud text-sub"
                  }`}
                >
                  {i.status === 0 ? t.unused : i.status === 1 ? t.used : t.expired}
                </span>
                {i.status === 1 && i.used_by && (
                  <span className="ml-1 text-xs text-sub">→ {i.used_by}</span>
                )}
              </td>
              <td className="text-xs text-sub">
                {i.status === 0 ? (i.email ?? "—") : (i.email ?? "—")}
              </td>
              <td className="num text-xs text-sub">{fmtDate(i.expires_at)}</td>
              <td>
                {i.status === 0 && (
                  <div className="flex gap-2">
                    <button
                      type="button"
                      className="cmgmt-act"
                      disabled={busyId === i.id}
                      onClick={() => onCopy(i)}
                    >
                      {t.copy}
                    </button>
                    <button
                      type="button"
                      className="cmgmt-act cmgmt-act--ok"
                      disabled={busyId === i.id}
                      onClick={() => onSend(i)}
                    >
                      {i.emailed ? t.resend : t.sendEmail}
                    </button>
                  </div>
                )}
              </td>
            </tr>
          ))}
        </tbody>
      </table>
    </div>
  );
}

/** 发送邀请邮件弹层（收件地址填完才可发送） */
export function SendEmailDialog({
  sendFor,
  sendEmail,
  setSendEmail,
  busy,
  onClose,
  onSend,
}: {
  sendFor: InviteItem;
  sendEmail: string;
  setSendEmail: (v: string) => void;
  busy: boolean;
  onClose: () => void;
  onSend: () => void;
}) {
  const { dict } = useI18n();
  const t = dict.invites;
  return (
    <div
      className="fixed inset-0 z-50 flex items-center justify-center bg-black/40 p-4"
      role="dialog"
      aria-modal="true"
      onClick={(e) => e.target === e.currentTarget && onClose()}
    >
      <div className="w-full max-w-md rounded-[var(--r-lg)] border border-line bg-[var(--baozi-paper)] p-5 shadow-[var(--shadow-card)]">
        <h2 className="font-display text-lg">{t.sendEmailTitle}</h2>
        <p className="mt-1 break-all text-xs text-sub">
          {t.sendEmailNote}
          <br />
          {sendFor.code}
        </p>
        <input
          type="email"
          value={sendEmail}
          onChange={(e) => setSendEmail(e.target.value)}
          placeholder={t.emailPlaceholder}
          className="mt-3 w-full rounded-[var(--r-md)] border border-line bg-[var(--surface-card)] px-3 py-2 text-sm"
        />
        <div className="mt-4 flex justify-end gap-2">
          <button
            type="button"
            className="min-h-[40px] rounded-full px-4 text-sm text-sub hover:text-ink"
            onClick={onClose}
          >
            {t.cancel}
          </button>
          <button
            type="button"
            className="min-h-[40px] rounded-full bg-sky-deep px-5 text-sm text-white disabled:opacity-50"
            disabled={busy || !sendEmail.includes("@")}
            onClick={onSend}
          >
            {t.send}
          </button>
        </div>
      </div>
    </div>
  );
}
