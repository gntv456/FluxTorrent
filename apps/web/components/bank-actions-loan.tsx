"use client";

import { useI18n } from "@/i18n/client";
import type { BankOverview } from "@/lib/data";

/** 贷款服务面板（从 components/bank-actions.tsx 按域拆出）：
 *  有贷（本息合计/到期/一次结清）与无贷（金额/档期日息/申请）两态。
 *  动作回调由 BankCard 注入。 */

type Dict = ReturnType<typeof useI18n>["dict"];

export function BankLoanPanel({
  dict,
  currency,
  ov,
  busy,
  loanAmount,
  setLoanAmount,
  loanTerm,
  setLoanTerm,
  onLoanApply,
  onLoanRepay,
}: {
  dict: Dict;
  currency: string;
  ov: BankOverview;
  busy: boolean;
  loanAmount: string;
  setLoanAmount: (v: string) => void;
  loanTerm: number;
  setLoanTerm: (v: number) => void;
  onLoanApply: () => void;
  onLoanRepay: () => void;
}) {
  const fmt = (n: number) => n.toLocaleString();
  const bp = (b: number) => `${(b / 100).toFixed(2)}%`;
  const daysLeft = (iso: string) =>
    Math.max(0, Math.ceil((new Date(iso).getTime() - Date.now()) / 86_400_000));
  return (
    <section className="flex flex-col gap-3 rounded-[var(--r-md)] border border-line bg-[var(--surface-card)] p-4 shadow-[var(--shadow-card)]">
      <h2 className="font-bold">{dict.bank.loanService}</h2>
      {ov.loan ? (
        <>
          <div className="flex flex-wrap items-baseline gap-2 text-sm">
            <span className="num font-bold">
              {dict.bank.loanDebt}:{" "}
              {fmt(ov.loan.remaining + ov.loan.accrued_interest)}
            </span>
            <span className="text-sub">
              {dict.bank.dueIn.replace("{n}", String(daysLeft(ov.loan.due_at)))}{" "}
              · {bp(ov.loan.daily_rate_bp)}/日
            </span>
          </div>
          <p className="text-xs text-sub">
            {dict.bank.loanPayoffNote
              .replace("{p}", fmt(ov.loan.remaining))
              .replace("{i}", fmt(ov.loan.accrued_interest))}
          </p>
          <button
            type="button"
            disabled={busy}
            onClick={onLoanRepay}
            className="min-h-[44px] rounded-full bg-sun px-5 text-sm text-ink disabled:opacity-50"
          >
            {dict.bank.repayAll}
          </button>
        </>
      ) : (
        <>
          <div className="flex flex-wrap items-center gap-2">
            <input
              type="number"
              min={1}
              value={loanAmount}
              onChange={(e) => setLoanAmount(e.target.value)}
              placeholder={dict.bank.loanAmount}
              className="min-h-[44px] w-36 rounded-[var(--r-sm)] border border-line bg-[var(--surface-card)] px-3"
            />
            <select
              value={loanTerm}
              onChange={(e) => setLoanTerm(Number(e.target.value))}
              className="min-h-[44px] rounded-[var(--r-sm)] border border-line bg-[var(--surface-card)] px-2"
            >
              {[7, 30, 90, 180, 365].map((t) => {
                const r = ov.loan_rates.find((f) => f.term_days === t);
                return (
                  <option key={t} value={t}>
                    {dict.bank.termDays.replace("{n}", String(t))}
                    {r ? ` · ${bp(r.daily_rate_bp)}/日` : ""}
                  </option>
                );
              })}
            </select>
            <button
              type="button"
              disabled={busy || !loanAmount}
              onClick={onLoanApply}
              className="min-h-[44px] rounded-full bg-sky-deep px-5 text-sm text-white disabled:opacity-50"
            >
              {dict.bank.loanApply}
            </button>
          </div>
          <p className="text-xs text-sub">
            {dict.bank.loanRule
              .replace("{min}", fmt(ov.limits.min_loan))
              .replace("{max}", fmt(ov.max_loan))
              .replace("{magic}", currency)}
          </p>
        </>
      )}
      <p className="text-xs text-sub">{dict.bank.loanOverdueNote}</p>
    </section>
  );
}
