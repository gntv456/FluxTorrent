"use client";

import { useCallback, useEffect, useState } from "react";
import { api, ApiError, hasSessionCookie } from "@/lib/api-client";
import { useI18n } from "@/i18n/client";
import { dateLocale, fmt } from "@/i18n/config";
import { FundingPanel } from "@/components/funding-panel";
import { LedgerTable, type LedgerRow } from "./_inner-ledger";
import { DonateTiers, type DonationTier } from "./_parts/donate-tiers";

interface DonatePlan {
  id: number;
  plan_type: "quota" | "upload" | "vip";
  title: string;
  reward: string | null;
  price_usd: number;
  sort: number;
}

interface DonateState {
  wallet_usd: number;
  vip_until: string | null;
  plans: DonatePlan[];
  ledger: LedgerRow[];
  /** E7：捐赠回馈档位（空数组=未启用） */
  donation_tiers?: DonationTier[];
  /** U4 §12.1：通道可用性（provider 配置齐全或 FLUX_DEMO）；false 时充值区显示「通道未开通」 */
  payment_enabled?: boolean;
}

const QUICK_AMOUNTS = [10, 20, 30, 50, 66];

/** 捐赠中心（馒头 donate 口径）：储值钱包 + 充值 + 三区套餐订购
 *  （储值流水表格拆到 _inner-ledger.tsx） */
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
  // 挂载门控（同 /my-spark）：hasSessionCookie() 在渲染期读 document.cookie，
  // SSR 恒 false → 服务端出「请先登录」而客户端出完整页面 → React #418。
  const [mounted, setMounted] = useState(false);

  const load = useCallback(() => {
    api.get<DonateState>("/api/v1/donate/state").then(setSt).catch(() => setSt(null));
  }, []);
  useEffect(load, [load]);
  useEffect(() => setMounted(true), []);

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

  const loggedIn = mounted && hasSessionCookie();

  if (!mounted) {
    // 与 SSR 一致的中性态（必须在 hooks 之后）
    return (
      <div className="flex flex-col gap-4">
        <h1 className="font-display text-2xl">{t.title}</h1>
        <p className="baozi-panel p-4 text-sm text-sub">
          {dict.common.loading}
        </p>
      </div>
    );
  }

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
          className="btn btn-sm btn-primary"
          disabled={busy || !affordable}
          onClick={() =>
            guard(async () => {
              const r = await api.post<{ plan: string }>(
                "/api/v1/donate/order",
                { plan_id: p.id },
              );
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
      <div className="pghd">
        <div>
          <div className="pg-eyebrow">Donation</div>
          <h1 className="font-display text-2xl">{t.title}</h1>
        </div>
        <span className="sub">{t.subtitle}</span>
        <div className="aside">
          {vipActive && (
            <div className="donate-vip-badge">
              VIP ·{" "}
              {t.vipUntil.replace(
                "{d}",
                new Date(st!.vip_until!).toLocaleDateString(
                  dateLocale(locale),
                ),
              )}
            </div>
          )}
        </div>
      </div>
      {msg && (
        <p className="rounded-[var(--r-md)] bg-sky-soft p-3 text-sm text-ink">
          {msg}
        </p>
      )}

      {/* 储值钱包 */}
      <section className="baozi-panel">
        <div className="baozi-panel__head">
          <h2>{t.wallet}</h2>
        </div>
        <div className="flex flex-col gap-3 p-4">
          <p className="text-xs text-sub">{t.walletNote}</p>
          <div className="flex flex-wrap items-center gap-3">
            <div>
              <p className="text-xs text-sub">{t.balance}</p>
              <p className="num text-2xl font-bold text-deep">
                {(st?.wallet_usd ?? 0).toFixed(2)}
                <span className="text-sm"> USD</span>
              </p>
            </div>
            <div className="ml-auto flex gap-2">
              <button
                className="btn btn-sm btn-primary"
                onClick={() => setTopupOpen(true)}
              >
                {t.btnTopup}
              </button>
              <button
                className="btn btn-sm"
                onClick={() => setLedgerOpen((v) => !v)}
              >
                {t.btnLedger}
              </button>
            </div>
          </div>
        </div>
      </section>

      {/* 套餐三区 */}
      <section className="baozi-panel">
        <div className="baozi-panel__head">
          <h2>{t.quotaZone}</h2>
        </div>
        <div className="grid grid-cols-2 gap-3 p-4 sm:grid-cols-3 lg:grid-cols-4">
          {quotaPlans.map((p) => <PlanCard key={p.id} p={p} />)}
        </div>
      </section>

      <section className="baozi-panel">
        <div className="baozi-panel__head">
          <h2>{t.uploadZone}</h2>
        </div>
        <div className="grid grid-cols-2 gap-3 p-4 sm:grid-cols-3 lg:grid-cols-4">
          {uploadPlans.map((p) => <PlanCard key={p.id} p={p} />)}
        </div>
      </section>

      <section className="baozi-panel">
        <div className="baozi-panel__head">
          <h2>{t.vipZone}</h2>
          <span className="text-xs text-sub">{t.vipHelp}</span>
        </div>
        <div className="grid grid-cols-2 gap-3 p-4 sm:grid-cols-3 lg:grid-cols-4">
          {vipPlans.map((p) => <PlanCard key={p.id} p={p} />)}
        </div>
      </section>

      <DonateTiers tiers={st?.donation_tiers ?? []} t={t} />

      {/* 众筹免费（0078）：凑火花挂限时免费 */}
      <FundingPanel />

      {/* 流水 */}
      {ledgerOpen && <LedgerTable rows={st?.ledger} />}

      {/* 充值弹层 */}
      {topupOpen && (
        <div className="baozi-panel flex flex-col gap-3 p-4">
          <h3 className="text-base font-bold text-ink">{t.topupTitle}</h3>
          {st && st.payment_enabled === false && (
            <p className="rounded-md border border-warn/40 bg-warn/10 px-3 py-2 text-sm">
              {t.channelClosed}
            </p>
          )}
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
            disabled={busy || amount < 10 || amount > 66 || st?.payment_enabled === false}
            onClick={() =>
              guard(async () => {
                const r = await api.post<{ order_no?: string; pay_url?: string; wallet_usd?: number }>(
                  "/api/v1/donate/topup",
                  { amount_usd: amount, channel },
                );
                if (r.pay_url && r.order_no) {
                  // 真实网关订单：跳收银台，回跳后由 ?order= 轮询状态（U4 §12.1）
                  window.location.assign(r.pay_url);
                } else {
                  // FLUX_DEMO 模拟充值（无 pay_url）
                  flash(t.toppedUp.replace("{n}", (r.wallet_usd ?? 0).toFixed(2)));
                  setTopupOpen(false);
                }
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
