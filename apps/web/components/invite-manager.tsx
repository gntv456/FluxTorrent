"use client";

import { useCallback, useEffect, useRef, useState } from "react";
import { api, ApiError } from "@/lib/api-client";
import { useI18n } from "@/i18n/client";
import { InviteQuotaPanel } from "@/components/invite-quota-panel";
import { InviteeList, type Invitee } from "@/components/invite-invitees";
import { InviteList, SendEmailDialog } from "@/components/invite-manager-parts";
import { clientUuid } from "@/lib/client-uuid";
import { toast } from "@/components/ui/toast";
import { ConfirmDialog } from "@/components/ui/confirm-dialog";
import { Pager } from "@/components/ui/pager";
import { ErrorPanel } from "@/components/ui/error-panel";

export interface InviteItem {
  id: number;
  code: string;
  status: number; // 0 未用 1 已用 2 已过期 3 已撤销（后端按 expires_at 折算）
  used_by: string | null;
  expires_at: string;
  email?: string | null;
  emailed?: boolean | null;
}

/** GET /invites 响应（0204 分页信封） */
interface InvitesEnvelope {
  items: InviteItem[];
  total: number;
  page: number;
  per: number;
}

/** GET /invites/status：配额 + 兑换价 + 计数（用于禁用态与顶部概览） */
export interface InviteStatus {
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
  const [total, setTotal] = useState(0);
  const [page, setPage] = useState(1);
  const [filter, setFilter] = useState<number | null>(null);
  const listSeqRef = useRef(0);
  const [invitees, setInvitees] = useState<Invitee[] | null>(null);
  const [inviteeTotal, setInviteeTotal] = useState(0);
  const [inviteePage, setInviteePage] = useState(1);
  const [inviteeErr, setInviteeErr] = useState<unknown>(null);

  /** 被邀请人分页信封（2026-10-16 后端补分页，与邀请记录同口径 per=50） */
  interface InviteeEnvelope {
    items: Invitee[];
    total: number;
    page: number;
    per: number;
  }

  const refreshInvitees = useCallback(async (p = 1) => {
    setInviteeErr(null);
    try {
      const r = await api.get<InviteeEnvelope>(
        `/api/v1/invites/invitees?page=${p}`,
      );
      setInvitees(r.items);
      setInviteeTotal(r.total);
      setInviteePage(r.page);
    } catch (e) {
      setInviteeErr(e);
      setInvitees([]);
    }
  }, []);
  // W-19：撤销确认（ConfirmDialog；原 window.confirm + flash 行内提示
  // 一并退役——反馈走 toast 协议）
  const [askRevoke, setAskRevoke] = useState<InviteItem | null>(null);
  const [busy, setBusy] = useState(false);
  const [busyId, setBusyId] = useState<number | null>(null);
  const [copiedId, setCopiedId] = useState<number | null>(null);
  const [sendFor, setSendFor] = useState<InviteItem | null>(null);
  const [sendEmail, setSendEmail] = useState("");

  // 反馈协议（DS-PLAN-01 W-10）：flash/flashErr 改走全局 toast
  const flash = useCallback((m: string) => {
    toast.success(m);
  }, []);
  const flashErr = useCallback((m: string) => {
    toast.error(m);
  }, []);

  const refresh = useCallback(async (p = 1) => {
    const seq = ++listSeqRef.current;
    const statusQ = filter === null ? "" : `&status=${filter}`;
    try {
      const [st, list] = await Promise.all([
        api.get<InviteStatus>("/api/v1/invites/status"),
        api.get<InvitesEnvelope>(
          `/api/v1/invites?page=${p}${statusQ}`,
        ),
      ]);
      if (seq !== listSeqRef.current) return;
      setStatus(st);
      setInvites(list.items);
      setTotal(list.total);
      setPage(list.page);
    } catch {
      if (seq !== listSeqRef.current) return;
      // P-314：置回 null（渲染侧 null 分支不宣称「暂无邀请码」）+ 走可见的 toast
      setStatus(null); setInvites(null); flashErr(dict.common.loadFailed);
    }
    // 被邀请人独立分页：不跟邀请记录的 page 联动，两张表各翻各的
    void refreshInvitees();
  }, [dict, filter, flashErr, refreshInvitees]);
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
        { idempotency_key: (inviteIdemRef.current ??= clientUuid()) },
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

  /** 撤销未用邀请码（0204 P0）：status 0→3；不返还配额 */
  async function revoke(inv: InviteItem) {
    setBusyId(inv.id);
    try {
      await api.del(`/api/v1/invites/${inv.id}`);
      flash(t.revoked);
      setAskRevoke(null);
      await refresh(page);
    } catch (e) {
      flashErr(e instanceof ApiError ? e.message : dict.common.networkError);
    } finally {
      setBusyId(null);
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
      {/* 配额概览 + 操作区（2026-10-16 外移到 invite-quota-panel.tsx） */}
      <InviteQuotaPanel
        status={status}
        t={t}
        currency={currency}
        levelOk={levelOk}
        quotaLeft={quotaLeft}
        canIssue={canIssue}
        busy={busy}
        filter={filter}
        onIssue={issue}
        onRedeem={redeem}
        onFilter={(f) => setFilter((cur) => (cur === f ? null : f))}
      />

      {/* 邀请码列表（空态与加载态由 InviteList 内部处理，面板外壳始终在） */}
      {invites === null ? null : (
        <InviteList
          invites={invites}
          filter={filter}
          onClearFilter={() => setFilter(null)}
          copiedId={copiedId}
          busyId={busyId}
          onCopy={(inv) => void copyCode(inv)}
          onSend={(inv) => {
            setSendFor(inv);
            setSendEmail(inv.email ?? "");
          }}
          onRevoke={(inv) => setAskRevoke(inv)}
        />
      )}
      {/* 分页（0204：50/页） */}
      {total > 50 && (
        <Pager
          layout="compact"
          page={page}
          pages={Math.ceil(total / 50)}
          posLabel={`${page} / ${Math.ceil(total / 50)}`}
          onStep={(p) => void refresh(p)}
        />
      )}

      {/* 被邀请人列表（0204 P0；per=50 独立分页，见组件注释） */}
      {inviteeErr !== null && (
        <ErrorPanel
          error={inviteeErr}
          onRetry={() => void refreshInvitees(inviteePage)}
        />
      )}
      {inviteeErr === null && invitees !== null && (
        <InviteeList
          invitees={invitees}
          t={t}
          total={inviteeTotal}
          page={inviteePage}
          onPage={refreshInvitees}
          emptyTitle={dict.ui.emptyTitle}
          emptyDesc={dict.ui.emptyDesc}
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

      {/* W-19：撤销确认（danger；不返还配额故需显式表态） */}
      <ConfirmDialog
        open={askRevoke !== null}
        onClose={() => setAskRevoke(null)}
        onConfirm={() => {
          if (askRevoke) void revoke(askRevoke);
        }}
        tone="danger"
        title={t.revoke}
        message={t.confirmRevoke}
        busy={askRevoke !== null && busyId === askRevoke.id}
      />
    </div>
  );
}
