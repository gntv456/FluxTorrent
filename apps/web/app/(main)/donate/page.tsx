"use client";

import { useCallback, useEffect, useState } from "react";
import { api, ApiError } from "@/lib/api-client";
import { useI18n } from "@/i18n/client";
import { dateLocale } from "@/i18n/config";

interface DonatePlan {
  id: number;
  plan_type: "quota" | "upload" | "vip";
  title: string;
  reward: string | null;
  price_usd: number;
  sort: number;
}

interface LedgerRow {
  id: number;
  kind: "topup" | "order";
  amount_usd: number;
  balance_after: number;
  note: string | null;
  created_at: string;
}

interface DonateState {
  wallet_usd: number;
  vip_until: string | null;
  plans: DonatePlan[];
  ledger: LedgerRow[];
}

const QUICK_AMOUNTS = [10, 20, 30, 50, 66];

/** 捐赠中心（馒头 donate 口径）：储值钱包 + 充值 + 三区套餐订购 */
export default function DonatePage() {
  const { dict, locale } = useI18n();
  const t = dict.donate;
  const [st, setSt] = useState<DonateState | null>(null);
  const [msg, setMsg] = useState<string | null>(null);
  const [busy, setBusy] = useState(false);
  // 充值弹层
  const [topupOpen, setTopupOpen] = useState(false);
  const [amount, setAmount] = useState(10);
  const [channel, setChannel] = useState<"alipay" | "wechat">("alipay");
  // 流水弹层
  const [ledgerOpen, setLedgerOpen] = useState(false);

  const load = useCallback(() => {
    api.get<DonateState>("/api/v1/donate/state").then(setSt).catch(() => setSt(null));
  }, []);
  useEffect(load, [load]);

  function flash(m: string) {
    setMsg(m);
    setTimeout(() => setMsg(null), 3500);
  }
  async function guard(fn: () => Promise<void>) {
    setBusy(true);
    try {
      await fn();
      load();
    } catch (e) {
      flash(e instanceof ApiError ? e.message : dict.common.networkError);
    } finally {
      setBusy(false);
    }
  }

  const loggedIn = typeof window !== "undefined" && !!localStorage.getItem("flux.token");
  if (!loggedIn) {
    return (
      <div className="flex flex-col gap-4">
        <h1 className="font-display text-2xl">{t.title}</h1>
        <p className="baozi-panel p-4 text-sm text-sub">{dict.common.pleaseLogin}</p>
      </div>
    );
  }

  const quotaPlans = st?.plans.filter((p) => p.plan_type === "quota") ?? [];
  const uploadPlans = st?.plans.filter((p) => p.plan_type === "upload") ?? [];
  const vipPlans = st?.plans.filter((p) => p.plan_type === "vip") ?? [];
  const vipActive = st?.vip_until && new Date(st.vip_until) > new Date();

  function PlanCard({ p }: { p: DonatePlan }) {
    const affordable = (st?.wallet_usd ?? 0) >= p.price_usd;
    return (
      <div className="donate-plan">
        <p className="donate-plan__title">{p.title}</p>
        {p.reward && <p className="donate-plan__reward">{p.reward}</p>}
        <p className="donate-plan__price num">{p.price_usd} USD</p>
        <button
          disabled={busy || !affordable}
          onClick={() =>
            guard(async () => {
              const r = await api.post<{ plan: string }>("/api/v1/donate/order", { plan_id: p.id });
              flash(t.ordered.replace("{plan}", r.plan));
            })
          }
        >
          {t.btnOrder}
        </button>
        {!affordable && (
          <p className="donate-plan__lack num">{t.lack.replace("{n}", (p.price_usd - (st?.wallet_usd ?? 0)).toFixed(2))}</p>
        )}
      </div>
    );
  }

  return (
    <div className="flex flex-col gap-4">
      <h1 className="font-display text-2xl">{t.title}</h1>
      {msg && <p className="rounded-[var(--r-md)] bg-sky-soft p-3 text-sm text-ink">{msg}</p>}

      {/* 储值钱包 */}
      <section className="baozi-panel flex flex-col gap-3 p-4">
        <h2 className="text-base font-bold text-ink">{t.wallet}</h2>
        <p className="text-xs text-sub">{t.walletNote}</p>
        <div className="flex flex-wrap items-center gap-3">
          <div>
            <p className="text-xs text-sub">{t.balance}</p>
            <p className="num text-2xl font-bold text-[var(--baozi-orange-dark)]">
              {(st?.wallet_usd ?? 0).toFixed(2)} <span className="text-sm">USD</span>
            </p>
          </div>
          {vipActive && (
            <div className="donate-vip-badge">
              VIP · {t.vipUntil.replace("{d}", new Date(st!.vip_until!).toLocaleDateString(dateLocale(locale)))}
            </div>
          )}
          <div className="ml-auto flex gap-2">
            <button className="baozi-button" onClick={() => setTopupOpen(true)}>
              {t.btnTopup}
            </button>
            <button
              className="min-h-[36px] rounded-full border border-line px-4 text-xs font-bold"
              onClick={() => setLedgerOpen((v) => !v)}
            >
              {t.btnLedger}
            </button>
          </div>
        </div>
      </section>

      {/* 套餐三区 */}
      <section className="flex flex-col gap-3">
        <table className="nexus-table">
          <tbody>
            <tr>
              <td className="colhead">
                <h2 className="font-display">{t.quotaZone}</h2>
              </td>
            </tr>
          </tbody>
        </table>
        <div className="grid grid-cols-2 gap-3 sm:grid-cols-3 lg:grid-cols-4">
          {quotaPlans.map((p) => <PlanCard key={p.id} p={p} />)}
        </div>
      </section>

      <section className="flex flex-col gap-3">
        <table className="nexus-table">
          <tbody>
            <tr>
              <td className="colhead">
                <h2 className="font-display">{t.uploadZone}</h2>
              </td>
            </tr>
          </tbody>
        </table>
        <div className="grid grid-cols-2 gap-3 sm:grid-cols-3 lg:grid-cols-4">
          {uploadPlans.map((p) => <PlanCard key={p.id} p={p} />)}
        </div>
      </section>

      <section className="flex flex-col gap-3">
        <table className="nexus-table">
          <tbody>
            <tr>
              <td className="colhead">
                <h2 className="font-display">
                  {t.vipZone}
                  <span className="ml-2 text-xs font-normal text-sub">{t.vipHelp}</span>
                </h2>
              </td>
            </tr>
          </tbody>
        </table>
        <div className="grid grid-cols-2 gap-3 sm:grid-cols-3 lg:grid-cols-4">
          {vipPlans.map((p) => <PlanCard key={p.id} p={p} />)}
        </div>
      </section>

      {/* 流水 */}
      {ledgerOpen && (
        <table className="nexus-table">
          <tbody>
            <tr>
              <td className="colhead" colSpan={4}>
                <h2 className="font-display">{t.ledgerTitle}</h2>
              </td>
            </tr>
            <tr>
              <td className="colhead">{t.ledgerKind}</td>
              <td className="colhead">{t.ledgerAmount}</td>
              <td className="colhead">{t.balance}</td>
              <td className="colhead">{t.ledgerAt}</td>
            </tr>
            {(st?.ledger ?? []).map((r) => (
              <tr key={r.id}>
                <td>{r.kind === "topup" ? t.kindTopup : t.kindOrder}{r.note ? ` · ${r.note}` : ""}</td>
                <td className={`num font-bold ${r.amount_usd >= 0 ? "text-success" : "text-danger"}`}>
                  {r.amount_usd >= 0 ? "+" : ""}{r.amount_usd.toFixed(2)}
                </td>
                <td className="num">{r.balance_after.toFixed(2)}</td>
                <td className="text-xs text-sub">{new Date(r.created_at).toLocaleString(dateLocale(locale))}</td>
              </tr>
            ))}
            {(!st || st.ledger.length === 0) && (
              <tr><td colSpan={4} className="py-6 text-center text-sub">{t.ledgerEmpty}</td></tr>
            )}
          </tbody>
        </table>
      )}

      {/* 充值弹层 */}
      {topupOpen && (
        <div className="baozi-panel flex flex-col gap-3 p-4">
          <h3 className="text-base font-bold text-ink">{t.topupTitle}</h3>
          <label>
            {t.topupAmount}
            <input
              type="number"
              min={10}
              max={66}
              step={1}
              value={amount}
              onChange={(e) => setAmount(Number(e.target.value))}
            />
          </label>
          <p className="text-xs text-sub">{t.topupRange}</p>
          <div className="flex flex-wrap gap-2">
            {QUICK_AMOUNTS.map((v) => (
              <button key={v} className={`min-h-[32px] rounded-full px-3 text-xs font-bold ${amount === v ? "bg-sky text-white" : "border border-line"}`} onClick={() => setAmount(v)}>
                {v} USD
              </button>
            ))}
          </div>
          <div>
            <p className="mb-1 text-xs text-sub">{t.payChannel}</p>
            <div className="flex gap-4">
              <label className="flex items-center gap-1 text-sm">
                <input type="radio" name="channel" checked={channel === "alipay"} onChange={() => setChannel("alipay")} />
                {t.alipay}
              </label>
              <label className="flex items-center gap-1 text-sm">
                <input type="radio" name="channel" checked={channel === "wechat"} onChange={() => setChannel("wechat")} />
                {t.wechat}
              </label>
            </div>
          </div>
          <button
            className="baozi-button self-start"
            disabled={busy || amount < 10 || amount > 66}
            onClick={() =>
              guard(async () => {
                const r = await api.post<{ wallet_usd: number }>("/api/v1/donate/topup", {
                  amount_usd: amount,
                  channel,
                });
                flash(t.toppedUp.replace("{n}", r.wallet_usd.toFixed(2)));
                setTopupOpen(false);
              })
            }
          >
            {t.btnPay}
          </button>
          <p className="text-xs text-sub">{t.payNote}</p>
        </div>
      )}
    </div>
  );
}
