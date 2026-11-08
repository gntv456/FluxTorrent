"use client";

import { createContext, useContext, useState } from "react";
import { api, ApiError } from "@/lib/api-client";
import { useI18n } from "@/i18n/client";
import { fmt } from "@/i18n/config";
import { ConfirmDialog } from "@/components/ui/confirm-dialog";

/** 0482f 工作组成员明细表（自 admin-jixiao.tsx 拆出守行数门禁）：
 *  每行带「发薪 / 移除」操作——仅未结算行（status=0）可动，已发薪/已撤销
 *  行只读。移除=撤销本期登记（确认框填理由）；发薪=替成员手动结算
 *  （确认框可改实发额，缺省岗位底薪）。 */

export interface OverviewMember {
  type_id: number;
  user_id: number;
  username: string;
  status: number; // 0=进行中 1=达标已发薪 2=未达标 3=撤销
  amount: number | null;
  bonus: number | null;
  metrics_at_settle: Record<string, number>;
  claim_id?: number;
}

/** 行内操作上下文：成员表（admin-jixiao.tsx）的行按钮通过它打开弹窗。 */
const RowActionsContext = createContext<{
  openRemove: (m: OverviewMember) => void;
  openSettle: (m: OverviewMember) => void;
  busy: boolean;
} | null>(null);

/** 行按钮组：仅 status=0（进行中）行渲染——已发薪/未达标/已撤销只读。 */
export function MemberRowActions({ m }: { m: OverviewMember }) {
  const at = useI18n().dict.adminJixiao;
  const ctx = useContext(RowActionsContext);
  if (!ctx || m.status !== 0 || !m.claim_id) return null;
  return (
    <span className="flex justify-end gap-1">
      <button
        type="button"
        className="cmgmt-act"
        disabled={ctx.busy}
        onClick={() => ctx.openSettle(m)}
      >
        {at.settleBtn}
      </button>
      <button
        type="button"
        className="cmgmt-act cmgmt-act--danger"
        disabled={ctx.busy}
        onClick={() => ctx.openRemove(m)}
      >
        {at.removeBtn}
      </button>
    </span>
  );
}

/** 操作面板：包裹成员表提供行按钮上下文 + 承载两个确认弹窗。 */
export function MemberActionsPanel({
  children,
  load,
  flash,
}: {
  children: React.ReactNode;
  load: () => Promise<void>;
  flash: (m: string) => void;
}) {
  const { dict } = useI18n();
  const at = dict.adminJixiao;
  const [busy, setBusy] = useState(false);
  const [removing, setRemoving] = useState<OverviewMember | null>(null);
  const [reason, setReason] = useState("");
  const [paying, setPaying] = useState<OverviewMember | null>(null);
  const [payAmount, setPayAmount] = useState("");

  async function remove() {
    if (!removing) return;
    setBusy(true);
    try {
      await api.post("/api/v1/admin/jixiao/remove", {
        claim_id: removing.claim_id,
        ...(reason.trim() ? { reason: reason.trim() } : {}),
      });
      flash(fmt(at.removedMsg, { user: removing.username }));
      setRemoving(null);
      setReason("");
      await load();
    } catch (e) {
      flash(e instanceof ApiError ? e.message : at.memberActionFail);
    } finally {
      setBusy(false);
    }
  }

  async function settle() {
    if (!paying) return;
    setBusy(true);
    try {
      const r = await api.post<{ amount: number }>(
        "/api/v1/admin/jixiao/settle",
        {
          claim_id: paying.claim_id,
          ...(payAmount.trim()
            ? { amount_override: Number(payAmount.trim()) }
            : {}),
        },
      );
      flash(
        fmt(at.settledMsg, {
          amount: r.amount.toLocaleString(),
          user: paying.username,
        }),
      );
      setPaying(null);
      setPayAmount("");
      await load();
    } catch (e) {
      flash(e instanceof ApiError ? e.message : at.memberActionFail);
    } finally {
      setBusy(false);
    }
  }

  return (
    <RowActionsContext.Provider
      value={{
        openRemove: (m) => {
          setRemoving(m);
          setReason("");
        },
        openSettle: (m) => {
          setPaying(m);
          setPayAmount("");
        },
        busy,
      }}
    >
      {children}

      {/* 移除确认（0482f）：可填理由，落审计 */}
      <ConfirmDialog
        open={removing !== null}
        onClose={() => {
          if (!busy) setRemoving(null);
        }}
        onConfirm={() => void remove()}
        title={at.removeTitle}
        message={fmt(at.removeMsg, { user: removing?.username ?? "" })}
        busy={busy}
        confirmLabel={at.removeConfirm}
      >
        <label className="mt-2 flex items-center justify-between gap-3 text-sm">
          <span className="text-sub">{at.removeReasonPh}</span>
          <input
            type="text"
            value={reason}
            onChange={(e) => setReason(e.target.value)}
            disabled={busy}
            className="h-8 w-48 rounded-[var(--r-sm)] border border-line
              bg-transparent px-2 text-right text-sm"
          />
        </label>
      </ConfirmDialog>

      {/* 手动发薪确认（0482f）：缺省底薪可改 */}
      <ConfirmDialog
        open={paying !== null}
        onClose={() => {
          if (!busy) setPaying(null);
        }}
        onConfirm={() => void settle()}
        title={at.settleTitle}
        message={fmt(at.settleMsg, {
          user: paying?.username ?? "",
          base: (paying?.amount ?? 0).toLocaleString(),
        })}
        busy={busy}
        confirmLabel={at.settleConfirm}
      >
        <label className="mt-2 flex items-center justify-between gap-3 text-sm">
          <span className="text-sub">{at.settleAmountLabel}</span>
          <input
            type="number"
            min="0"
            value={payAmount}
            onChange={(e) => setPayAmount(e.target.value)}
            disabled={busy}
            placeholder={String(paying?.amount ?? 0)}
            className="h-8 w-36 rounded-[var(--r-sm)] border border-line
              bg-transparent px-2 text-right text-sm"
          />
        </label>
      </ConfirmDialog>
    </RowActionsContext.Provider>
  );
}
