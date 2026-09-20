"use client";

import { useI18n } from "@/i18n/client";
import type { BankOverview } from "@/lib/data";

/** 存款服务面板（从 components/bank-actions.tsx 按域拆出）：
 *  定期（金额/档期利率/存入）+ 活期（余额/复利/存取）+ 各自规则文案。
 *  动作回调由 BankCard 注入。 */

type Dict = ReturnType<typeof useI18n>["dict"];

export const TERMS = [7, 30, 90, 180, 365];

export function BankDepositPanel({
  dict,
  currency,
  ov,
  busy,
  amount,
  setAmount,
  term,
  setTerm,
  demandAmount,
  setDemandAmount,
  onDeposit,
  onDemandIn,
  onDemandOut,
}: {
  dict: Dict;
  currency: string;
  ov: BankOverview;
  busy: boolean;
  amount: string;
  setAmount: (v: string) => void;
  term: number;
  setTerm: (v: number) => void;
  demandAmount: string;
  setDemandAmount: (v: string) => void;
  onDeposit: () => void;
  onDemandIn: () => void;
  onDemandOut: () => void;
}) {
  const fmt = (n: number) => n.toLocaleString();
  const bp = (b: number) => `${(b / 100).toFixed(2)}%`;
  return (
    <section className="flex flex-col gap-3 rounded-[var(--r-md)] border border-line bg-[var(--surface-card)] p-4 shadow-[var(--shadow-card)]">
      <h2 className="font-bold">{dict.bank.depositService}</h2>
      <div className="flex flex-wrap items-center gap-2">
        <input
          type="number"
          min={1}
          value={amount}
          onChange={(e) => setAmount(e.target.value)}
          placeholder={dict.bank.amountPlaceholder}
          className="min-h-[44px] w-40 rounded-[var(--r-sm)] border border-line bg-[var(--surface-card)] px-3"
        />
        <select
          value={term}
          onChange={(e) => setTerm(Number(e.target.value))}
          className="min-h-[44px] rounded-[var(--r-sm)] border border-line bg-[var(--surface-card)] px-2"
        >
          {TERMS.map((t) => {
            const r = ov.fixed_rates.find((f) => f.term_days === t);
            return (
              <option key={t} value={t}>
                {dict.bank.termDays.replace("{n}", String(t))}
                {r ? ` · ${(r.annual_rate * 100).toFixed(0)}%` : ""}
              </option>
            );
          })}
        </select>
        <button
          type="button"
          disabled={busy || !amount}
          onClick={onDeposit}
          className="min-h-[44px] rounded-full bg-sky-deep px-5 text-sm text-white disabled:opacity-50"
        >
          {dict.bank.deposit}
        </button>
      </div>
      <p className="text-xs text-sub">
        {dict.bank.fixedRule
          .replace("{magic}", currency)
          .replace("{min}", fmt(ov.limits.min_deposit))
          .replace("{max}", fmt(ov.limits.max_deposit))
          .replace("{p}", bp(ov.limits.penalty_bp))}
      </p>

      {/* 活期账户 */}
      <h3 className="mt-1 text-sm font-bold">{dict.bank.demandTitle}</h3>
      <div className="flex flex-wrap items-baseline gap-2 text-sm">
        <span className="num font-bold">{fmt(ov.demand.balance)}</span>
        <span className="text-sub">
          {dict.bank.demandRate}: {bp(ov.demand.daily_rate_bp)}/日 · {dict.bank.demandCompound}
        </span>
      </div>
      <div className="flex flex-wrap items-center gap-2">
        <input
          type="number"
          min={1}
          value={demandAmount}
          onChange={(e) => setDemandAmount(e.target.value)}
          placeholder={dict.bank.demandAmount}
          className="min-h-[44px] w-40 rounded-[var(--r-sm)] border border-line bg-[var(--surface-card)] px-3"
        />
        <button
          type="button"
          disabled={busy || !demandAmount}
          onClick={onDemandIn}
          className="min-h-[44px] rounded-full bg-mint/60 px-5 text-sm disabled:opacity-50"
        >
          {dict.bank.demandIn}
        </button>
        <button
          type="button"
          disabled={busy || !demandAmount || ov.demand.balance <= 0}
          onClick={onDemandOut}
          className="min-h-[44px] rounded-full border border-line px-5 text-sm text-sky-deep disabled:opacity-50"
        >
          {dict.bank.demandOut}
        </button>
      </div>
      <p className="text-xs text-sub">
        {dict.bank.demandRule
          .replace("{min}", fmt(ov.limits.min_demand))
          .replace("{magic}", currency)}
      </p>
    </section>
  );
}
