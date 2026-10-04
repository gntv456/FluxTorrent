"use client";

import { useCallback, useEffect, useRef, useState } from "react";
import { api, ApiError } from "@/lib/api-client";
import { useI18n } from "@/i18n/client";
import type {
  BankDeposit,
  BankInterestRecord,
  BankLoanHistoryItem,
  BankOverview,
} from "@/lib/data";
import { Stat, DepositRow } from "@/components/bank-actions-parts";
import { BankDepositPanel } from "@/components/bank-actions-deposit";
import { BankLoanPanel, LoanHistoryRow } from "@/components/bank-actions-loan";
import { clientUuid } from "@/lib/client-uuid";

// 银行系统（火花银行对齐）：活期复利 + 定期 + 贷款 + 资产概览。
// 拆出：资产格/存款行 @/components/bank-actions-parts、
// 存款面板 @/components/bank-actions-deposit、
// 贷款面板与贷款历史行 @/components/bank-actions-loan。

const fmt = (n: number) => n.toLocaleString();

export function BankCard() {
  const { dict, currency } = useI18n();
  const [ov, setOv] = useState<BankOverview | null>(null);
  const [ovFailed, setOvFailed] = useState(false);
  const [deposits, setDeposits] = useState<BankDeposit[]>([]);
  const [loanHistory, setLoanHistory] = useState<BankLoanHistoryItem[]>([]);
  const [interest, setInterest] = useState<BankInterestRecord[]>([]);
  const [amount, setAmount] = useState("");
  const [term, setTerm] = useState(30);
  const [demandAmount, setDemandAmount] = useState("");
  const [loanAmount, setLoanAmount] = useState("");
  const [loanTerm, setLoanTerm] = useState(30);
  const [repayAmount, setRepayAmount] = useState("");
  const [msg, setMsg] = useState<string | null>(null);
  const [busy, setBusy] = useState(false);
  // 部分还款幂等键：一次输入会话一个键，成功后轮换（§8.2 幂等语义）
  const repayIdemRef = useRef<string>("");

  const refresh = useCallback(async () => {
    setOvFailed(false);
    try {
      const o = await api.get<BankOverview>("/api/v1/bank/overview");
      setOv(o);
    } catch {
      setOv(null);
      setOvFailed(true);
    }
    try {
      setDeposits(await api.get<BankDeposit[]>("/api/v1/bank/deposits"));
    } catch {
      setDeposits([]);
    }
    try {
      setLoanHistory(
        await api.get<BankLoanHistoryItem[]>("/api/v1/bank/loans"),
      );
    } catch {
      setLoanHistory([]);
    }
    try {
      setInterest(
        await api.get<BankInterestRecord[]>("/api/v1/bank/interest/records"),
      );
    } catch {
      setInterest([]);
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
      await api.post("/api/v1/bank/deposit", {
        amount: Number(amount),
        term_days: term,
      });
      setAmount("");
      return "✓";
    });
  const withdraw = (depositId: number) =>
    act(async () => {
      const r = await api.post<{ penalty: number; matured: boolean }>(
        "/api/v1/bank/withdraw",
        {
          deposit_id: depositId,
        },
      );
      return r.matured
        ? "✓"
        : dict.bank.penaltyTaken
            .replace("{n}", fmt(r.penalty))
            .replace("{magic}", currency);
    });
  const demandIn = () =>
    act(async () => {
      await api.post("/api/v1/bank/demand/deposit", {
        amount: Number(demandAmount),
      });
      setDemandAmount("");
      return "✓";
    });
  const demandOut = () =>
    act(async () => {
      await api.post("/api/v1/bank/demand/withdraw", {
        amount: Number(demandAmount),
      });
      setDemandAmount("");
      return "✓";
    });
  const loanApply = () =>
    act(async () => {
      await api.post("/api/v1/bank/loan/apply", {
        amount: Number(loanAmount),
        term_days: loanTerm,
      });
      setLoanAmount("");
      return "✓";
    });
  const loanRepay = (partial?: number) =>
    act(async () => {
      if (partial !== undefined) {
        repayIdemRef.current ||= `web-repay-${clientUuid()}`;
        const r = await api.post<{
          paid: number;
          principal: number;
          interest: number;
          settled: boolean;
        }>("/api/v1/bank/loan/repay", {
          amount: partial,
          idempotency_key: repayIdemRef.current,
        });
        if (r.settled) {
          setRepayAmount("");
          repayIdemRef.current = "";
          return dict.bank.repaid
            .replace("{n}", fmt(r.paid))
            .replace("{magic}", currency);
        }
        return dict.bank.repaidPartial
          .replace("{p}", fmt(r.principal))
          .replace("{i}", fmt(r.interest))
          .replace("{n}", fmt(r.paid))
          .replace("{magic}", currency);
      }
      const r = await api.post<{ paid: number }>("/api/v1/bank/loan/repay", {});
      return dict.bank.repaid
        .replace("{n}", fmt(r.paid))
        .replace("{magic}", currency);
    });

  // overview 拉取失败且无任何本地数据：显式错误态 + 重试（旧版静默空态像「模块缺失」）
  if (ov === null) {
    if (ovFailed && deposits.length === 0 && loanHistory.length === 0) {
      return (
        <div
          role="alert"
          className="flex flex-col items-center gap-3 rounded-[var(--r-md)] border border-line bg-[var(--surface-card)] px-4 py-10 text-center shadow-[var(--shadow-card)]"
        >
          <p className="text-sm text-sub">{dict.common.loadFailed}</p>
          <button
            type="button"
            disabled={busy}
            onClick={() => refresh()}
            className="min-h-[44px] rounded-full bg-sky-deep px-6 text-sm text-white disabled:opacity-50"
          >
            {dict.common.retry}
          </button>
        </div>
      );
    }
    return deposits.length > 0 || loanHistory.length > 0 ? (
      <ul className="flex flex-col gap-2">
        {loanHistory.map((l) => (
          <LoanHistoryRow key={`loan-${l.id}`} l={l} />
        ))}
        {deposits.map((d) => (
          <DepositRow
            key={d.id}
            d={d}
            dict={dict}
            busy={busy}
            onWithdraw={withdraw}
          />
        ))}
      </ul>
    ) : (
      <p className="text-xs text-sub">{dict.bank.emptyDeposits}</p>
    );
  }

  return (
    <div className="flex flex-col gap-4">
      {msg && (
        <p
          role="alert"
          className="rounded-[var(--r-sm)] bg-mint/20 px-3 py-2 text-sm text-sky-deep"
        >
          {msg}
        </p>
      )}

      {/* 站点银行条：运营概览 + 结息健康 */}
      <div className="flex flex-wrap items-center gap-x-4 gap-y-1 rounded-[var(--r-md)] border border-line bg-[var(--surface-card)] px-4 py-2 text-xs text-sub shadow-[var(--shadow-card)]">
        <span className="font-bold text-ink">{dict.bank.siteOverview}</span>
        <span>
          {dict.bank.siteDemand}: {fmt(ov.site.demand_total)}（
          {ov.site.demand_count}）
        </span>
        <span>
          {dict.bank.siteFixed}: {fmt(ov.site.fixed_active_total)}（
          {ov.site.fixed_count}）
        </span>
        <span>
          {dict.bank.siteLoan}: {fmt(ov.site.loan_outstanding_total)}（
          {ov.site.loan_count}）
        </span>
        <span>
          {dict.bank.siteTodayRecords}: {fmt(ov.site.today_interest_records)}
        </span>
        <span
          className={`ml-auto sticker ${ov.site.settle_healthy ? "bg-mint/40 text-ink" : "bg-sun/70 text-ink"}`}
        >
          {ov.site.settle_healthy
            ? dict.bank.settleOk
            : dict.bank.settlePending}
        </span>
      </div>

      {/* 资产概览 */}
      <div className="grid grid-cols-2 gap-2 sm:grid-cols-3 lg:grid-cols-6">
        <Stat label={dict.bank.totalAsset} value={fmt(ov.total_asset)} />
        <Stat label={dict.bank.netAsset} value={fmt(ov.net_asset)} />
        <Stat
          label={dict.bank.sparkBalance.replace("{magic}", currency)}
          value={fmt(ov.spark_balance)}
        />
        <Stat label={dict.bank.demandBalance} value={fmt(ov.demand.balance)} />
        <Stat
          label={dict.bank.fixedActive}
          value={`${fmt(ov.fixed.active_total)}（${ov.fixed.active_count}）`}
        />
        <Stat label={dict.bank.loanDebt} value={fmt(ov.loan_outstanding)} />
      </div>

      <div className="grid gap-4 lg:grid-cols-2">
        {/* 存款服务 */}
        <BankDepositPanel
          dict={dict}
          currency={currency}
          ov={ov}
          busy={busy}
          amount={amount}
          setAmount={setAmount}
          term={term}
          setTerm={setTerm}
          demandAmount={demandAmount}
          setDemandAmount={setDemandAmount}
          onDeposit={deposit}
          onDemandIn={demandIn}
          onDemandOut={demandOut}
        />

        {/* 贷款服务 */}
        <BankLoanPanel
          dict={dict}
          currency={currency}
          ov={ov}
          busy={busy}
          loanAmount={loanAmount}
          setLoanAmount={setLoanAmount}
          loanTerm={loanTerm}
          setLoanTerm={setLoanTerm}
          repayAmount={repayAmount}
          setRepayAmount={setRepayAmount}
          onLoanApply={loanApply}
          onLoanRepay={loanRepay}
        />
      </div>

      {/* 贷款历史 */}
      {loanHistory.length > 0 && (
        <div className="flex flex-col gap-2">
          <h2 className="text-sm font-bold text-sub">
            {dict.bank.loanHistoryTitle}
          </h2>
          <ul className="flex flex-col gap-2">
            {loanHistory.map((l) => (
              <LoanHistoryRow key={l.id} l={l} />
            ))}
          </ul>
        </div>
      )}

      {/* 利息流水 */}
      {interest.length > 0 && (
        <div className="flex flex-col gap-2">
          <h2 className="text-sm font-bold text-sub">
            {dict.bank.interestRecordsTitle}
          </h2>
          <ul className="flex flex-col gap-1">
            {interest.map((r) => (
              <li
                key={r.id}
                className="flex flex-wrap items-center justify-between gap-2 rounded-[var(--r-sm)] border border-line bg-[var(--surface-card)] px-3 py-2 text-xs"
              >
                <span className="sticker num bg-mint/30 text-ink">
                  {r.kind === "demand"
                    ? dict.bank.kindDemand
                    : r.kind === "fixed"
                      ? dict.bank.kindFixed
                      : dict.bank.kindLoan}
                </span>
                <span className="num font-bold">
                  +{fmt(r.amount)} {currency}
                </span>
                <span className="text-sub">
                  {(r.rate_bp / 100).toFixed(2)}% ·{" "}
                  {new Date(r.calc_date).toLocaleDateString()}
                </span>
              </li>
            ))}
          </ul>
        </div>
      )}

      {/* 定期存款列表 */}
      {deposits.length > 0 && (
        <ul className="flex flex-col gap-2">
          {deposits.map((d) => (
            <DepositRow
              key={d.id}
              d={d}
              dict={dict}
              busy={busy}
              onWithdraw={withdraw}
            />
          ))}
        </ul>
      )}
    </div>
  );
}
