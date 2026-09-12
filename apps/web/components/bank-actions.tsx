"use client";

import { useCallback, useEffect, useState } from "react";
import { api, ApiError } from "@/lib/api-client";
import { useI18n } from "@/i18n/client";
import type { BankDeposit, BankOverview } from "@/lib/data";

const TERMS = [7, 30, 90, 180, 365];
const fmt = (n: number) => n.toLocaleString();
const bp = (b: number) => `${(b / 100).toFixed(2)}%`;

/** 银行系统（火花银行对齐）：活期复利 + 定期 + 贷款 + 资产概览 */
export function BankCard({ loginToView }: { loginToView: string }) {
  const { dict } = useI18n();
  const [ov, setOv] = useState<BankOverview | null>(null);
  const [deposits, setDeposits] = useState<BankDeposit[]>([]);
  const [amount, setAmount] = useState("");
  const [term, setTerm] = useState(30);
  const [demandAmount, setDemandAmount] = useState("");
  const [loanAmount, setLoanAmount] = useState("");
  const [loanTerm, setLoanTerm] = useState(30);
  const [msg, setMsg] = useState<string | null>(null);
  const [busy, setBusy] = useState(false);

  const refresh = useCallback(async () => {
    try {
      const o = await api.get<BankOverview>("/api/v1/bank/overview");
      setOv(o);
    } catch {
      setOv(null);
    }
    try {
      setDeposits(await api.get<BankDeposit[]>("/api/v1/bank/deposits"));
    } catch {
      setDeposits([]);
    }
  }, []);
  useEffect(() => {
    refresh();
  }, [refresh]);

  async function act(fn: () => Promise<string>) {
    setBusy(true);
    setMsg(null);
    try {
      setMsg(await fn());
      refresh();
    } catch (e) {
      setMsg(e instanceof ApiError ? e.message : dict.common.networkError);
    } finally {
      setBusy(false);
    }
  }

  const deposit = () =>
    act(async () => {
      await api.post("/api/v1/bank/deposit", { amount: Number(amount), term_days: term });
      setAmount("");
      return "✓";
    });
  const withdraw = (depositId: number) =>
    act(async () => {
      const r = await api.post<{ penalty: number; matured: boolean }>("/api/v1/bank/withdraw", {
        deposit_id: depositId,
      });
      return r.matured ? "✓" : dict.bank.penaltyTaken.replace("{n}", fmt(r.penalty));
    });
  const demandIn = () =>
    act(async () => {
      await api.post("/api/v1/bank/demand/deposit", { amount: Number(demandAmount) });
      setDemandAmount("");
      return "✓";
    });
  const demandOut = () =>
    act(async () => {
      await api.post("/api/v1/bank/demand/withdraw", { amount: Number(demandAmount) });
      setDemandAmount("");
      return "✓";
    });
  const loanApply = () =>
    act(async () => {
      await api.post("/api/v1/bank/loan/apply", { amount: Number(loanAmount), term_days: loanTerm });
      setLoanAmount("");
      return "✓";
    });
  const loanRepay = () =>
    act(async () => {
      const r = await api.post<{ paid: number }>("/api/v1/bank/loan/repay", {});
      return dict.bank.repaid.replace("{n}", fmt(r.paid));
    });

  if (ov === null) {
    return deposits.length > 0 ? (
      <ul className="flex flex-col gap-2">
        {deposits.map((d) => (
          <DepositRow key={d.id} d={d} dict={dict} busy={busy} onWithdraw={withdraw} />
        ))}
      </ul>
    ) : (
      <p className="text-xs text-sub">{loginToView}</p>
    );
  }

  const daysLeft = (iso: string) =>
    Math.max(0, Math.ceil((new Date(iso).getTime() - Date.now()) / 86_400_000));

  return (
    <div className="flex flex-col gap-4">
      {msg && (
        <p role="alert" className="rounded-[var(--r-sm)] bg-mint/20 px-3 py-2 text-sm text-sky-deep">
          {msg}
        </p>
      )}

      {/* 站点银行条：运营概览 + 结息健康 */}
      <div className="flex flex-wrap items-center gap-x-4 gap-y-1 rounded-[var(--r-md)] border border-line bg-[var(--surface-card)] px-4 py-2 text-xs text-sub shadow-[var(--shadow-card)]">
        <span className="font-bold text-ink">{dict.bank.siteOverview}</span>
        <span>
          {dict.bank.siteDemand}: {fmt(ov.site.demand_total)}（{ov.site.demand_count}）
        </span>
        <span>
          {dict.bank.siteFixed}: {fmt(ov.site.fixed_active_total)}（{ov.site.fixed_count}）
        </span>
        <span>
          {dict.bank.siteLoan}: {fmt(ov.site.loan_outstanding_total)}（{ov.site.loan_count}）
        </span>
        <span>
          {dict.bank.siteTodayRecords}: {fmt(ov.site.today_interest_records)}
        </span>
        <span
          className={`ml-auto sticker ${ov.site.settle_healthy ? "bg-mint/40 text-ink" : "bg-sun/70 text-ink"}`}
        >
          {ov.site.settle_healthy ? dict.bank.settleOk : dict.bank.settlePending}
        </span>
      </div>

      {/* 资产概览 */}
      <div className="grid grid-cols-2 gap-2 sm:grid-cols-3 lg:grid-cols-6">
        <Stat label={dict.bank.totalAsset} value={fmt(ov.total_asset)} />
        <Stat label={dict.bank.netAsset} value={fmt(ov.net_asset)} />
        <Stat label={dict.bank.sparkBalance} value={fmt(ov.spark_balance)} />
        <Stat label={dict.bank.demandBalance} value={fmt(ov.demand.balance)} />
        <Stat
          label={dict.bank.fixedActive}
          value={`${fmt(ov.fixed.active_total)}（${ov.fixed.active_count}）`}
        />
        <Stat label={dict.bank.loanDebt} value={fmt(ov.loan_outstanding)} />
      </div>

      <div className="grid gap-4 lg:grid-cols-2">
        {/* 存款服务 */}
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
              onClick={deposit}
              className="min-h-[44px] rounded-full bg-sky-deep px-5 text-sm text-white disabled:opacity-50"
            >
              {dict.bank.deposit}
            </button>
          </div>
          <p className="text-xs text-sub">
            {dict.bank.fixedRule
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
              onClick={demandIn}
              className="min-h-[44px] rounded-full bg-mint/60 px-5 text-sm disabled:opacity-50"
            >
              {dict.bank.demandIn}
            </button>
            <button
              type="button"
              disabled={busy || !demandAmount || ov.demand.balance <= 0}
              onClick={demandOut}
              className="min-h-[44px] rounded-full border border-line px-5 text-sm text-sky-deep disabled:opacity-50"
            >
              {dict.bank.demandOut}
            </button>
          </div>
          <p className="text-xs text-sub">
            {dict.bank.demandRule.replace("{min}", fmt(ov.limits.min_demand))}
          </p>
        </section>

        {/* 贷款服务 */}
        <section className="flex flex-col gap-3 rounded-[var(--r-md)] border border-line bg-[var(--surface-card)] p-4 shadow-[var(--shadow-card)]">
          <h2 className="font-bold">{dict.bank.loanService}</h2>
          {ov.loan ? (
            <>
              <div className="flex flex-wrap items-baseline gap-2 text-sm">
                <span className="num font-bold">
                  {dict.bank.loanDebt}: {fmt(ov.loan.remaining + ov.loan.accrued_interest)}
                </span>
                <span className="text-sub">
                  {dict.bank.dueIn.replace("{n}", String(daysLeft(ov.loan.due_at)))} ·{" "}
                  {bp(ov.loan.daily_rate_bp)}/日
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
                onClick={loanRepay}
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
                  {TERMS.map((t) => {
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
                  onClick={loanApply}
                  className="min-h-[44px] rounded-full bg-sky-deep px-5 text-sm text-white disabled:opacity-50"
                >
                  {dict.bank.loanApply}
                </button>
              </div>
              <p className="text-xs text-sub">
                {dict.bank.loanRule
                  .replace("{min}", fmt(ov.limits.min_loan))
                  .replace("{max}", fmt(ov.max_loan))}
              </p>
            </>
          )}
          <p className="text-xs text-sub">{dict.bank.loanOverdueNote}</p>
        </section>
      </div>

      {/* 定期存款列表 */}
      {deposits.length > 0 && (
        <ul className="flex flex-col gap-2">
          {deposits.map((d) => (
            <DepositRow key={d.id} d={d} dict={dict} busy={busy} onWithdraw={withdraw} />
          ))}
        </ul>
      )}
      <p className="text-xs text-sub">{loginToView}</p>
    </div>
  );
}

function Stat({ label, value }: { label: string; value: string }) {
  return (
    <div className="rounded-[var(--r-sm)] border border-line bg-[var(--surface-card)] px-3 py-2">
      <div className="text-xs text-sub">{label}</div>
      <div className="num text-sm font-bold">{value}</div>
    </div>
  );
}

type Dict = ReturnType<typeof useI18n>["dict"];

function DepositRow({
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
        {d.status === 0 && !matured ? ` · ${dict.bank.daysLeft.replace("{n}", String(days))}` : ""}
      </span>
      <span className={`sticker num ${d.status === 0 ? "bg-sun text-ink" : "bg-mint/30 text-ink"}`}>
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
