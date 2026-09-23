"use client";

import { useI18n } from "@/i18n/client";
import type { BankLoanHistoryItem, BankOverview } from "@/lib/data";

/** 贷款服务面板（从 components/bank-actions.tsx 按域拆出）：
 *  有贷（本息合计/到期/部分还款/一次结清）与无贷（金额/档期日息/申请）两态；
 *  底部贷款历史列表（进行中 + 已结清）。动作回调由 BankCard 注入。 */

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
  repayAmount,
  setRepayAmount,
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
  repayAmount: string;
  setRepayAmount: (v: string) => void;
  onLoanApply: () => void;
  onLoanRepay: (partial?: number) => void;
}) {
  const fmt = (n: number) => n.toLocaleString();
  const bp = (b: number) => `${(b / 100).toFixed(2)}%`;
  const daysLeft = (iso: string) =>
    Math.max(0, Math.ceil((new Date(iso).getTime() - Date.now()) / 86_400_000));
  const payoff = ov.loan ? ov.loan.remaining + ov.loan.accrued_interest : 0;
  return (
    <section className="flex flex-col gap-3 rounded-[var(--r-md)] border border-line bg-[var(--surface-card)] p-4 shadow-[var(--shadow-card)]">
      <h2 className="font-bold">{dict.bank.loanService}</h2>
      {ov.loan ? (
        <>
          <div className="flex flex-wrap items-baseline gap-2 text-sm">
            <span className="num font-bold">
              {dict.bank.loanDebt}: {fmt(payoff)}
            </span>
            <span className="text-sub">
              {dict.bank.dueIn.replace("{n}", String(daysLeft(ov.loan.due_at)))}{" "}
              · {bp(ov.loan.daily_rate_bp)}/日
            </span>
            <span className="ml-auto text-xs text-sub">
              {dict.bank.loanRemainingQuota.replace(
                "{n}",
                fmt(Math.max(0, ov.max_loan)),
              )}
            </span>
          </div>
          <p className="text-xs text-sub">
            {dict.bank.loanPayoffNote
              .replace("{p}", fmt(ov.loan.remaining))
              .replace("{i}", fmt(ov.loan.accrued_interest))}
          </p>
          <div className="flex flex-wrap items-center gap-2">
            <input
              type="number"
              min={1}
              max={payoff}
              value={repayAmount}
              onChange={(e) => setRepayAmount(e.target.value)}
              placeholder={dict.bank.repayAmount}
              className="min-h-[44px] w-36 rounded-[var(--r-sm)] border border-line bg-[var(--surface-card)] px-3"
            />
            <button
              type="button"
              disabled={busy || !repayAmount || Number(repayAmount) <= 0}
              onClick={() => onLoanRepay(Number(repayAmount))}
              className="min-h-[44px] rounded-full border border-sky-deep px-5 text-sm text-sky-deep disabled:opacity-50"
            >
              {dict.bank.repayPartial}
            </button>
            <button
              type="button"
              disabled={busy}
              onClick={() => onLoanRepay()}
              className="min-h-[44px] rounded-full bg-sun px-5 text-sm text-ink disabled:opacity-50"
            >
              {dict.bank.repayAll}
            </button>
          </div>
          <p className="text-xs text-sub">
            {dict.bank.repayPartialNote.replace("{magic}", currency)}
          </p>
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

/** 贷款历史行：金额 + 期限 + 状态角标 + 到期/结清时间 */
export function LoanHistoryRow({ l }: { l: BankLoanHistoryItem }) {
  const { dict } = useI18n();
  const fmt = (n: number) => n.toLocaleString();
  const active = l.status === "active" || l.status === "defaulted";
  return (
    <li className="flex flex-wrap items-center justify-between gap-2 rounded-[var(--r-md)] border border-line bg-[var(--surface-card)] p-4 shadow-[var(--shadow-card)]">
      <span className="num font-bold">{fmt(l.amount)}</span>
      <span className="text-sm text-sub">
        {dict.bank.termDays.replace("{n}", String(l.term_days))}
        {active
          ? ` · ${dict.bank.loanDebt}: ${fmt(l.remaining + l.accrued_interest)}`
          : ` · ${dict.bank.loanInterestTotal}: ${fmt(l.accrued_interest)}`}
      </span>
      <span className="text-xs text-sub">
        {active
          ? `${dict.bank.maturity}: ${new Date(l.due_at).toLocaleDateString()}`
          : `${dict.bank.loanPaidAt}: ${l.paid_at ? new Date(l.paid_at).toLocaleDateString() : "—"}`}
      </span>
      <span
        className={`sticker num ${l.status === "paid" ? "bg-mint/30 text-ink" : l.status === "defaulted" ? "bg-tomato/40 text-ink" : "bg-sun/70 text-ink"}`}
      >
        {l.status === "paid"
          ? dict.bank.loanStatusPaid
          : l.status === "defaulted"
            ? dict.bank.loanStatusDefaulted
            : dict.bank.loanStatusActive}
      </span>
    </li>
  );
}
