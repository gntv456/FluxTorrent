"use client";

import { useEffect, useState } from "react";
import { api, ApiError } from "@/lib/api-client";
import { useI18n } from "@/i18n/client";
import type { BankDeposit } from "@/lib/data";

const TERMS = [7, 30, 90, 180, 365];

/** 银行系统（包子站 bank.php 同款）：存入赚利息，到期取回本息 */
export function BankCard({ loginToView }: { loginToView: string }) {
  const { dict } = useI18n();
  const [deposits, setDeposits] = useState<BankDeposit[] | null>(null);
  const [amount, setAmount] = useState("");
  const [term, setTerm] = useState(30);
  const [msg, setMsg] = useState<string | null>(null);
  const [busy, setBusy] = useState(false);

  async function refresh() {
    try {
      setDeposits(await api.get<BankDeposit[]>("/api/v1/bank/deposits"));
    } catch {
      setDeposits([]);
    }
  }
  useEffect(() => {
    refresh();
  }, []);

  async function deposit() {
    setBusy(true);
    setMsg(null);
    try {
      await api.post("/api/v1/bank/deposit", { amount: Number(amount), term_days: term });
      setAmount("");
      setMsg("✓");
      refresh();
    } catch (e) {
      setMsg(e instanceof ApiError ? e.message : dict.common.networkError);
    } finally {
      setBusy(false);
    }
  }

  async function withdraw(depositId: number) {
    setBusy(true);
    setMsg(null);
    try {
      await api.post("/api/v1/bank/withdraw", { deposit_id: depositId });
      setMsg("✓");
      refresh();
    } catch (e) {
      setMsg(e instanceof ApiError ? e.message : dict.common.networkError);
    } finally {
      setBusy(false);
    }
  }

  if (deposits === null) return null;
  return (
    <div className="flex flex-col gap-4">
      <div className="rounded-[var(--r-md)] border border-line bg-[var(--surface-card)] p-4 shadow-[var(--shadow-card)]">
        <h2 className="font-bold">{dict.bank.deposit}</h2>
        <div className="mt-2 flex flex-wrap items-center gap-2">
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
            className="min-h-[44px] rounded-[var(--r-sm)] border border-line bg-[var(--surface-card)] px-3"
          >
            {TERMS.map((t) => (
              <option key={t} value={t}>
                {fmtTerm(t, dict)}
              </option>
            ))}
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
        {msg && (
          <p role="alert" className="mt-2 text-sm text-sky-deep">
            {msg}
          </p>
        )}
      </div>

      {deposits.length > 0 && (
        <ul className="flex flex-col gap-2">
          {deposits.map((d) => (
            <li
              key={d.id}
              className="flex flex-wrap items-center justify-between gap-2 rounded-[var(--r-md)] border border-line bg-[var(--surface-card)] p-4 shadow-[var(--shadow-card)]"
            >
              <span className="num font-bold">{d.amount.toLocaleString()}</span>
              <span className="text-sm text-sub">
                {fmtTerm(d.term_days, dict)} · {dict.bank.interest}: {d.interest.toLocaleString()}
              </span>
              <span
                className={`sticker num ${
                  d.status === 0 ? "bg-sun text-ink" : "bg-mint/30 text-ink"
                }`}
              >
                {d.status === 0 ? dict.bank.locked : dict.bank.matured}
              </span>
              {d.status === 0 && (
                <button
                  type="button"
                  disabled={busy}
                  onClick={() => withdraw(d.id)}
                  className="min-h-[36px] rounded-full border border-line px-4 text-sm text-sky-deep disabled:opacity-50"
                >
                  {dict.bank.withdraw}
                </button>
              )}
            </li>
          ))}
        </ul>
      )}
      <p className="text-xs text-sub">{loginToView}</p>
    </div>
  );
}

function fmtTerm(days: number, dict: ReturnType<typeof useI18n>["dict"]): string {
  return dict.bank.termDays.replace("{n}", String(days));
}
