"use client";

import { useEffect, useState } from "react";

import { Modal } from "@/components/modal";
import { api } from "@/lib/api-client";
import { useI18n } from "@/i18n/client";
import { dateLocale } from "@/i18n/config";
import type { InviteItem } from "@/components/invite-manager";
import { Badge } from "@/components/ui/badge";
import { EmptyState } from "@/components/ui/empty-state";

/** 邀请管理展示件（从 invite-manager.tsx 按域拆出）：
 *  邀请码列表表格 + 发送邀请邮件弹层（数据装载与动作留在原文件）。 */

/** 邀请码列表：复制 / 状态角标 / 发送到邮箱 / 重发 / 撤销 / 过期展示 */
export function InviteList({
  invites,
  copiedId,
  busyId,
  onCopy,
  onSend,
  onRevoke,
}: {
  invites: InviteItem[];
  copiedId: number | null;
  busyId: number | null;
  onCopy: (inv: InviteItem) => void;
  onSend: (inv: InviteItem) => void;
  onRevoke: (inv: InviteItem) => void;
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
  // 2026-10-17 对齐样图集 § tplInvite（panelFlat）：原先表格裸挂在页面上，
  // 与全站「面板 + 标题栏」的观感断层。用既有 .brand-panel 原语包一层，
  // 标题走 t.listTitle（「我的邀请码」），不新增体系。
  return (
    <section className="brand-panel">
      <div className="brand-panel__head">
        <h2>{t.listTitle}</h2>
      </div>
      {invites.length === 0 ? (
        // 空态走统一件（设计系统决策 § 加载/空/错误：空态统一组件）；
        // 原先由上层渲染一行裸 <p>，连面板外壳一起消失。
        <div className="p-4">
          <EmptyState icon="mail" title={t.empty} compact />
        </div>
      ) : (
      <div className="brand-wide-table-scroll">
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
                <Badge
                  tone={
                    i.status === 0
                      ? "warn"
                      : i.status === 1
                        ? "success"
                        : "neutral"
                  }
                  variant="soft"
                >
                  {i.status === 0
                    ? t.unused
                    : i.status === 1
                      ? t.used
                      : i.status === 3
                        ? t.revokedTag
                        : t.expired}
                </Badge>
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
                    <button
                      type="button"
                      className="cmgmt-act cmgmt-act--danger"
                      disabled={busyId === i.id}
                      onClick={() => onRevoke(i)}
                    >
                      {t.revoke}
                    </button>
                  </div>
                )}
              </td>
            </tr>
          ))}
          </tbody>
        </table>
      </div>
      )}
    </section>
  );
}

/** 发送邀请邮件弹层（收件地址填完才可发送；受限后缀实时禁发 + 提示） */
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
  const { dict, locale } = useI18n();
  const t = dict.invites;
  // 邮箱后缀策略（0282）：与后端同口径的前端预判——填到受限后缀时
  // 禁用发送按钮并说明原因，而不是等点发送才被 400。
  const [policy, setPolicy] = useState<{
    policy: string;
    list: string[];
  } | null>(null);
  useEffect(() => {
    api
      .get<{ policy: string; list: string[] }>("/api/v1/auth/email-policy")
      .then(setPolicy)
      .catch(() => setPolicy(null)); // 拉不到=不预判，交给后端兜底
  }, []);
  const suffix = sendEmail.includes("@")
    ? sendEmail.slice(sendEmail.lastIndexOf("@") + 1).trim().toLowerCase()
    : "";
  const policyBlocked =
    policy !== null &&
    policy.list.length > 0 &&
    suffix !== "" &&
    (policy.policy === "allow_list"
      ? !policy.list.includes(suffix)
      : policy.policy === "deny_list" && policy.list.includes(suffix));
  const blockedHint = policyBlocked
    ? policy.policy === "allow_list"
      ? t.suffixNotAllowed
      : t.suffixBanned
    : null;
  return (
    <Modal open onClose={onClose} title={t.sendEmailTitle}>
      <div className="w-full max-w-md">
        <p className="mt-1 break-all text-xs text-sub">
          {t.sendEmailNote.replace(
            "{date}",
            new Date(sendFor.expires_at).toLocaleString(
              dateLocale(locale),
              {
                month: "2-digit",
                day: "2-digit",
                hour: "2-digit",
                minute: "2-digit",
              },
            ),
          )}
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
        {blockedHint && (
          <p className="mt-2 text-xs text-danger" role="alert">
            {blockedHint}
          </p>
        )}
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
            disabled={busy || !sendEmail.includes("@") || policyBlocked}
            onClick={onSend}
          >
            {t.send}
          </button>
        </div>
      </div>
    </Modal>
  );
}
