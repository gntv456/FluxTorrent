"use client";

/** 固定抽签页·类型与领取表单（0520 拆件守 300 行门禁）。
 *  主组件在 ./_inner.tsx。 */

import { BTN_MD_SKY, INPUT_CARD } from "@/lib/ui-classes";
import { fmt } from "@/i18n/config";

export type ListRow = {
  code: string;
  title: string;
  status: string;
  trigger: string;
  drawAt: string | null;
  ticketPrice: number;
  voided: boolean;
  joined: boolean;
};

export type Prize = {
  id: number;
  sortOrder: number;
  name: string;
  quantity: number;
  rewardKind: string;
  rewardConfig: Record<string, unknown>;
};

export type Detail = {
  code: string;
  title: string;
  intro: string;
  status: string;
  trigger: string;
  startsAt: string | null;
  drawAt: string | null;
  fallbackAt: string | null;
  threshold: number | null;
  ticketPrice: number;
  participants: number;
  joined: boolean;
  drawMeta: Record<string, unknown> | null;
  prizes: Prize[];
  myPrize: {
    winnerId: number;
    prizeName: string;
    slotNo: number;
    status: string;
    rewardKind: string;
    claimStatus: string | null;
    rejectReason: string | null;
    submission: Record<string, unknown> | null;
  } | null;
  winners: {
    username: string;
    prizeName: string;
    slotNo: number;
    position: number;
    status: string;
  }[];
};

export function statusKey(st: string): string {
  const map: Record<string, string> = {
    pending: "statusPending",
    fulfilled: "statusFulfilled",
    satisfied: "statusSatisfied",
    failed: "statusFailed",
    claimed: "statusClaimed",
  };
  return map[st] ?? "statusPending";
}

/** 只有自定义奖需要领取资料；approved 之后不能再改写审核结果。 */
export function claimFormVisible(
  p: Detail["myPrize"],
): p is NonNullable<Detail["myPrize"]> {
  return p?.rewardKind === "custom"
    && p.claimStatus !== "approved"
    && p.claimStatus !== "expired";
}

/** 自定义奖的状态口径与发放类奖不同，不能直接用 statusKey。 */
export function prizeStatusText(
  p: NonNullable<Detail["myPrize"]>,
  t: Record<string, string>,
): string {
  if (p.rewardKind !== "custom") {
    return t[statusKey(p.status)] ?? t.statusPending;
  }
  switch (p.claimStatus) {
    case "approved":
      return t.claimApproved;
    case "pending_review":
      return t.claimSubmitted;
    case "rejected":
      return fmt(t.claimRejected, { reason: p.rejectReason ?? "" });
    case "expired":
      return t.claimExpired;
    default:
      return t.claimToSubmit;
  }
}

export function ClaimForm(props: {
  t: Record<string, string>;
  busy: boolean;
  value: string;
  onChange: (v: string) => void;
  onSubmit: () => void;
  submission: Record<string, unknown> | null;
}) {
  const already =
    props.submission && Object.keys(props.submission).length > 0;
  return (
    <div className="mt-3">
      <h3 className="text-sm font-bold">{props.t.claimTitle}</h3>
      {already && (
        <div className="mt-1 text-xs opacity-60">
          {props.t.claimSubmitted}
        </div>
      )}
      <textarea
        className={INPUT_CARD + " mt-1 h-20 w-full text-sm"}
        placeholder='{"field": "value"}'
        value={props.value}
        onChange={(e) => props.onChange(e.target.value)}
      />
      <button
        className={BTN_MD_SKY + " mt-2 w-full"}
        disabled={props.busy}
        onClick={props.onSubmit}
      >
        {props.t.claimSubmit}
      </button>
    </div>
  );
}
