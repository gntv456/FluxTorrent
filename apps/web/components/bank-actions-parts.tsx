"use client";

import type { BankDeposit } from "@/lib/data";
import { useI18n } from "@/i18n/client";

/** 银行辅助件（从 components/bank-actions.tsx 按域拆出）：
 *  资产概览 Stat 格子 + 定期存款行 DepositRow（含未到期支取提示）。
 *  主组件 BankCard 与存/取/贷动作留在原文件。 */

type Dict = ReturnType<typeof useI18n>["dict"];

export function Stat({ label, value }: { label: string; value: string }) {
  return (
    <div className="rounded-[var(--r-sm)] border border-line bg-[var(--surface-card)] px-3 py-2">
      <div className="text-xs text-sub">{label}</div>
      <div className="num text-sm font-bold">{value}</div>
    </div>
  );
}

export function DepositRow({
  d,
  dict,
  busy,
  onWithdraw,
}: {
  d: BankDeposit;
  dict: Dict;
  busy: boolean;
  onWithdraw: (id: number) => void;
}) {
  const matured = new Date(d.maturity_at).getTime() <= Date.now();
  const days = Math.max(
    0,
    Math.ceil((new Date(d.maturity_at).getTime() - Date.now()) / 86_400_000),
  );
  return (
    <li className="flex flex-wrap items-center justify-between gap-2 rounded-[var(--r-md)] border border-line bg-[var(--surface-card)] p-4 shadow-[var(--shadow-card)]">
      <span className="num font-bold">{d.amount.toLocaleString()}</span>
      <span className="text-sm text-sub">
        {dict.bank.termDays.replace("{n}", String(d.term_days))} ·{" "}
        {d.settle_mode === "daily"
          ? dict.bank.paidInterest
              .replace("{n}", d.interest.toLocaleString())
              .replace("{p}", d.paid_interest.toLocaleString())
          : `${dict.bank.interest}: ${d.interest.toLocaleString()}`}
      </span>
      <span className="text-xs text-sub">
        {dict.bank.maturity}: {new Date(d.maturity_at).toLocaleDateString()}
        {d.status === 0 && !matured
          ? ` · ${dict.bank.daysLeft.replace("{n}", String(days))}`
          : ""}
      </span>
      <span
        className={`sticker num ${d.status === 0 ? "bg-sun text-ink" : "bg-mint/30 text-ink"}`}
      >
        {d.status === 0 ? dict.bank.locked : dict.bank.matured}
      </span>
      {d.status === 0 && (
        <button
          type="button"
          disabled={busy}
          onClick={() => onWithdraw(d.id)}
          className="min-h-[36px] rounded-full border border-line px-4 text-sm text-sky-deep disabled:opacity-50"
        >
          {dict.bank.withdraw}
        </button>
      )}
    </li>
  );
}
