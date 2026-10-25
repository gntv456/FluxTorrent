"use client";

import {
  PANEL_MD_FLAT,
  BTN_MD_SKY,
  BTN_MD_GHOST_SKY,
} from "@/lib/ui-classes";
import type { InviteStatus } from "@/components/invite-manager";

/** 邀请配额概览 + 操作区（2026-10-16 自 invite-manager.tsx 按域外移，
 *  300 行门禁；顺带把两个 44px 按钮的 Tailwind 串抽成 ui-classes 常量）。
 *
 *  禁用态给文案而非让用户点了才报错：等级不足 / 配额用尽各有提示。
 *  数据装载与动作（issue / redeem）留在原组件，本件纯展示 + 回调。
 */
export function InviteQuotaPanel({
  status,
  t,
  currency,
  levelOk,
  quotaLeft,
  canIssue,
  busy,
  filter,
  onIssue,
  onRedeem,
  onFilter,
}: {
  status: InviteStatus | null;
  t: Record<string, string>;
  currency: string;
  levelOk: boolean;
  quotaLeft: number;
  canIssue: boolean;
  busy: boolean;
  filter: number | null;
  onIssue: () => void;
  onRedeem: () => void;
  onFilter: (status: number) => void;
}) {
  const issueLabelWith = (base: string, lvOk: boolean, left: number) => {
    if (!lvOk) return t.issueDisabledLevel;
    if (left === 0 && (status?.quota_extra ?? 0) === 0)
      return t.issueDisabledQuota;
    return base;
  };
  return (
    <>
      <div className="grid grid-cols-2 gap-3 md:grid-cols-5">
        <div
          className="rounded-[var(--r-md)] border border-line
            bg-[var(--surface-card)] p-4 md:col-span-2"
        >          <p className="text-xs text-sub">{t.quotaTitle}</p>
          <p className="mt-1 text-lg">
            <b className="num">{status?.quota_used ?? "—"}</b>
            <span className="text-sub">
              {" / "}{status?.quota_limit ?? "—"} {t.quotaUnit}
            </span>
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
        {/* 三块计数同时是列表筛选器（再点一次取消），角标数与筛选结果同口径 */}
        {[
          { label: t.statUnused, value: status?.unused, status: 0 },
          { label: t.statUsed, value: status?.used, status: 1 },
          { label: t.statExpired, value: status?.expired, status: 2 },
        ].map((s) => (
          <button
            key={s.status}
            type="button"
            aria-pressed={filter === s.status}
            title={t.filterHint}
            onClick={() => onFilter(s.status)}
            className={`${PANEL_MD_FLAT} cursor-pointer text-left
              transition-shadow ${
                filter === s.status
                  ? "ring-2 ring-sky-deep"
                  : "hover:ring-1 hover:ring-line"
              }`}
          >
            <p className="text-xs text-sub">{s.label}</p>
            <p className="mt-1 num text-lg font-bold">{s.value ?? "—"}</p>
          </button>
        ))}
      </div>
      <div className="flex flex-wrap items-center gap-2">
        <button
          type="button"
          disabled={busy || !canIssue}
          onClick={onIssue}
          title={
            !levelOk
              ? t.needClass
              : quotaLeft === 0
                ? t.issueDisabledQuota
                : undefined
          }
          className={BTN_MD_SKY}
        >
          {issueLabelWith(t.issue, levelOk, quotaLeft)}
        </button>
        {status?.redeem_price != null && (
          <button
            type="button"
            disabled={busy}
            onClick={onRedeem}
            className={BTN_MD_GHOST_SKY}
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
    </>
  );
}
