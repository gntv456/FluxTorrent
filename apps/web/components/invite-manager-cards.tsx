"use client";

import { useI18n } from "@/i18n/client";
import { dateLocale } from "@/i18n/config";
import { Badge } from "@/components/ui/badge";
import type { InviteItem } from "@/components/invite-manager";

/** 邀请码列表的**手机卡片**形态（2026-11-02）。
 *
 *  桌面是五列表格（邀请码/状态/发送到/到期/操作）；窄屏那张表塞进 390px
 *  会横向溢出 220px——右侧那列（发送到/到期）直接被切掉看不见，得横滑。
 *  卡片把每条邀请码摊成一块：码 + 状态 + 去向 + 到期 + 操作。
 */

/** 卡片壳（与站内其它卡片同底） */
const CARD_CLS =
  "rounded-[var(--r-md)] border border-[var(--compat-line)] " +
  "bg-[var(--compat-paper)] p-3";

export function InviteCards({
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
  onCopy: (i: InviteItem) => void;
  onSend: (i: InviteItem) => void;
  onRevoke: (i: InviteItem) => void;
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
    <ul className="flex flex-col gap-2 p-2 sm:hidden">
      {invites.map((i) => (
        <li key={i.id} className={CARD_CLS}>
          <div className="flex items-center gap-2">
            <button
              type="button"
              onClick={() => onCopy(i)}
              className="num font-mono text-[13px] font-bold text-sky-deep"
              title={i.code}
            >
              {copiedId === i.id ? t.copied : `${i.code.slice(0, 10)}••••`}
            </button>
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
          </div>
          <p className="mt-1 truncate text-xs text-sub">
            {i.status === 1 && i.used_by ? `→ ${i.used_by}` : (i.email ?? "—")}
          </p>
          <p className="mt-0.5 text-xs text-sub">
            {t.colExpires} {fmtDate(i.expires_at)}
          </p>
          {i.status === 0 && (
            <div className="mt-2 flex gap-2">
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
        </li>
      ))}
    </ul>
  );
}
