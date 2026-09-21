"use client";

import { PANEL_MD_FLAT } from "@/lib/ui-classes";

import { useCallback, useEffect, useRef, useState } from "react";
import { api, ApiError } from "@/lib/api-client";
import { useI18n } from "@/i18n/client";
import { InviteList, SendEmailDialog } from "@/components/invite-manager-parts";

export interface InviteItem {
  id: number;
  code: string;
  status: number; // 0 未用 1 已用 2 已过期（后端按 expires_at 折算）
  used_by: string | null;
  expires_at: string;
  email?: string | null;
  emailed?: boolean | null;
}

/** GET /invites/status：配额 + 兑换价 + 计数（用于禁用态与顶部概览） */
interface InviteStatus {
  class_id: number;
  quota_limit: number;
  quota_used: number;
  quota_extra: number;
  redeem_price: number | null;
  unused: number;
  used: number;
  expired: number;
}

/**
 * 邀请管理（NP invite.php 口径）：
 * 配额概览（周配额/额外配额/三种状态计数）+ 生成（等级与配额不足时禁用而非点了报错）
 * + 魔力兑换 + 邀请码列表（复制 / 发送到邮箱 / 重发 / 过期展示）。
 * （列表表格与发送邮件弹层拆到 invite-manager-parts.tsx）
 */
export function InviteManager() {
  const { dict, currency } = useI18n();
  const t = dict.invites;
  const inviteIdemRef = useRef<string | null>(null);
  const [status, setStatus] = useState<InviteStatus | null>(null);
  const [invites, setInvites] = useState<InviteItem[] | null>(null);
  const [msg, setMsg] = useState<string | null>(null);
  const [err, setErr] = useState<string | null>(null);
  const [busy, setBusy] = useState(false);
  const [busyId, setBusyId] = useState<number | null>(null);
  const [copiedId, setCopiedId] = useState<number | null>(null);
  const [sendFor, setSendFor] = useState<InviteItem | null>(null);
  const [sendEmail, setSendEmail] = useState("");

  const flash = useCallback((m: string) => {
    setMsg(m);
    setErr(null);
    setTimeout(() => setMsg(null), 3500);
  }, []);
  const flashErr = useCallback((m: string) => {
    setErr(m);
    setMsg(null);
    setTimeout(() => setErr(null), 5000);
  }, []);

  const refresh = useCallback(async () => {
    try {
      const [st, list] = await Promise.all([
        api.get<InviteStatus>("/api/v1/invites/status"),
        api.get<InviteItem[]>("/api/v1/invites"),
      ]);
      setStatus(st);
      setInvites(list);
    } catch {
      setStatus(null);
      setInvites([]);
    }
  }, []);
  useEffect(() => {
    refresh();
  }, [refresh]);

  async function issue() {
    setBusy(true);
    try {
      const r = await api.post<{ code: string }>("/api/v1/invites", {});
      flash(r.code);
      await refresh();
    } catch (e) {
      flashErr(e instanceof ApiError ? e.message : dict.common.networkError);
    } finally {
      setBusy(false);
    }
  }

  async function redeem() {
    setBusy(true);
    try {
      // 幂等键：每次点击生成新 UUID（与 /shop/buy 同口径，防网络重试双扣款）
      const r = await api.post<{ code?: string; replayed?: boolean }>(
        "/api/v1/invites/redeem",
        { idempotency_key: (inviteIdemRef.current ??= crypto.randomUUID()) },
      );
      flash(r.code ?? t.replayed);
      await refresh();
    } catch (e) {
      flashErr(e instanceof ApiError ? e.message : dict.common.networkError);
    } finally {
      setBusy(false);
    }
  }

  async function copyCode(inv: InviteItem) {
    try {
      await navigator.clipboard.writeText(inv.code);
      setCopiedId(inv.id);
      setTimeout(() => setCopiedId(null), 2000);
    } catch {
      flashErr(inv.code);
    }
  }

  async function sendMail() {
    if (!sendFor) return;
    setBusy(true);
    try {
      await api.post("/api/v1/invites/email", {
        invite_id: sendFor.id,
        email: sendEmail.trim(),
      });
      flash(t.sent);
      setSendFor(null);
      setSendEmail("");
      await refresh();
    } catch (e) {
      flashErr(e instanceof ApiError ? e.message : dict.common.networkError);
    } finally {
      setBusy(false);
    }
  }

  const levelOk = (status?.quota_limit ?? 0) > 0;
  const quotaLeft =
    status === null ? 0 : Math.max(0, status.quota_limit - status.quota_used);
  const canIssue = levelOk && (quotaLeft > 0 || (status?.quota_extra ?? 0) > 0);

  return (
    <div className="flex flex-col gap-4">
      {/* 配额概览卡 */}
      <div className="grid grid-cols-2 gap-3 md:grid-cols-5">
        <div className="rounded-[var(--r-md)] border border-line bg-[var(--surface-card)] p-4 md:col-span-2">
          <p className="text-xs text-sub">{t.quotaTitle}</p>
          <p className="mt-1 text-lg">
            <b className="num">{status?.quota_used ?? "—"}</b>
            <span className="text-sub"> / {status?.quota_limit ?? "—"} </span>
            <span className="text-xs text-sub">
              {t.quotaWeek}
              {status && status.quota_extra > 0 && (
                <span className="ml-2">
                  （{t.quotaExtra} <b className="num">{status.quota_extra}</b>）
                </span>
              )}
            </span>
          </p>
        </div>
        {[
          { label: t.statUnused, value: status?.unused },
          { label: t.statUsed, value: status?.used },
          { label: t.statExpired, value: status?.expired },
        ].map((s) => (
          <div
            key={s.label}
            className={PANEL_MD_FLAT}
          >
            <p className="text-xs text-sub">{s.label}</p>
            <p className="mt-1 num text-lg font-bold">{s.value ?? "—"}</p>
          </div>
        ))}
      </div>

      {/* 操作区 */}
      <div className="flex flex-wrap items-center gap-2">
        <button
          type="button"
          disabled={busy || !canIssue}
          onClick={issue}
          title={
            !levelOk
              ? t.needClass
              : quotaLeft === 0
                ? t.issueDisabledQuota
                : undefined
          }
          className="min-h-[44px] rounded-full bg-sky-deep px-5 text-sm text-white disabled:cursor-not-allowed disabled:opacity-50"
        >
          {issueLabelWith(t.issue, levelOk, quotaLeft)}
        </button>
        {status?.redeem_price != null && (
          <button
            type="button"
            disabled={busy}
            onClick={redeem}
            className="min-h-[44px] rounded-full border border-[var(--baozi-line)] px-5 text-sm text-sky-deep hover:border-[var(--baozi-orange)] hover:text-[var(--baozi-orange)] disabled:opacity-50"
          >
            {t.redeem.replace("{magic}", currency)}（
            {status.redeem_price.toLocaleString()}）
          </button>
        )}
      </div>
      {status?.redeem_price != null && (
        <p className="text-xs text-sub">
          {t.redeemNote.replace("{magic}", currency)}
        </p>
      )}
      {!levelOk && <p className="text-xs text-sub">{t.needClass}</p>}
      {msg && (
        <p
          role="alert"
          className="num break-all rounded-[var(--r-md)] bg-mint/30 p-3 text-sm text-ink"
        >
          {msg}
        </p>
      )}
      {err && (
        <p
          role="alert"
          className="rounded-[var(--r-md)] bg-sun/30 p-3 text-sm text-ink"
        >
          {err}
        </p>
      )}

      {/* 邀请码列表 */}
      {invites === null ? null : invites.length === 0 ? (
        <p className="py-6 text-center text-sub">{t.empty}</p>
      ) : (
        <InviteList
          invites={invites}
          copiedId={copiedId}
          busyId={busyId}
          onCopy={(inv) => void copyCode(inv)}
          onSend={(inv) => {
            setSendFor(inv);
            setSendEmail(inv.email ?? "");
          }}
        />
      )}

      {/* 发送邀请邮件弹层 */}
      {sendFor && (
        <SendEmailDialog
          sendFor={sendFor}
          sendEmail={sendEmail}
          setSendEmail={setSendEmail}
          busy={busy}
          onClose={() => setSendFor(null)}
          onSend={sendMail}
        />
      )}
    </div>
  );

  /** 生成按钮文案：等级不足/配额用尽时给禁用态提示，而非报错 */
  function issueLabelWith(base: string, lvOk: boolean, left: number): string {
    if (!lvOk) return t.issueDisabledLevel;
    if (left === 0 && (status?.quota_extra ?? 0) === 0)
      return t.issueDisabledQuota;
    return base;
  }
}
