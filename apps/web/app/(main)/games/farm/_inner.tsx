"use client";

import { useCallback, useEffect, useState } from "react";
import { api, ApiError } from "@/lib/api-client";
import { useI18n } from "@/i18n/client";
import { dateLocale, fmt, fmtCur } from "@/i18n/config";
import { BalanceBar, ResultFlash } from "@/components/game/game-kit";
import { FarmPlot, MarketCard, type Crop, type Plot } from "@/components/game/farm-field";
import Link from "next/link";

interface FarmData {
  window_start: number;
  next_refresh: number;
  crops: Crop[];
  plots: Plot[];
  slots: number;
}

/** 农场专注页：六块田 + 图鉴式行情（旧 /farm 已重定向到此） */
export default function FarmPage() {
  const { dict, locale, currency } = useI18n();
  const tf = dict.farm;
  const tg = dict.games;

  const [data, setData] = useState<FarmData | null>(null);
  const [me, setMe] = useState<{ balance: number; today_net: number; limit_left: number } | null>(
    null,
  );
  const [now, setNow] = useState<number | null>(null);
  const [msg, setMsg] = useState<{ kind: "win" | "lose" | "tie" | "jackpot"; text: string } | null>(
    null,
  );
  const [err, setErr] = useState<string | null>(null);
  const [busy, setBusy] = useState(false);
  const [picking, setPicking] = useState<number | null>(null);

  const refresh = useCallback(async () => {
    try {
      setData(await api.get<FarmData>("/api/v1/farm"));
    } catch (e) {
      setErr(
        e instanceof ApiError && e.code === 2001
          ? dict.errors[2001]
          : dict.common.loadFailed,
      );
    }
  }, [dict]);

  const loadMe = useCallback(async () => {
    try {
      const r = await api.get<{ me?: { balance: number; today_net: number; limit_left: number } }>(
        "/api/v1/games",
      );
      setMe(r.me ?? null);
    } catch {
      /* ignore */
    }
  }, []);

  useEffect(() => {
    void refresh();
    // 农场接口不带「我的」，从总览接口补余额/今日/剩余（下注条常驻可见）
    void loadMe();
  }, [refresh, loadMe]);

  // 倒计时 tick（SSR 不渲染时间，避免 hydration 不一致）
  useEffect(() => {
    setNow(Date.now());
    const id = window.setInterval(() => setNow(Date.now()), 1000);
    return () => window.clearInterval(id);
  }, []);

  async function act(
    path: string,
    body: unknown,
    ok: (d: never) => string,
    kindOf?: (d: never) => "win" | "jackpot",
  ) {
    setBusy(true);
    setErr(null);
    setMsg(null);
    try {
      const r = await api.post<Record<string, never>>(`/api/v1${path}`, body);
      setMsg({ kind: kindOf ? kindOf(r as never) : "win", text: ok(r as never) });
      setPicking(null);
      await refresh();
      void loadMe();
    } catch (e) {
      setErr(
        e instanceof ApiError
          ? (dict.errors[e.code] ?? e.message)
          : dict.common.networkError,
      );
    } finally {
      setBusy(false);
    }
  }

  if (!data) {
    return (
      <div className="py-16 text-center text-sub">{err ?? tf.loading}</div>
    );
  }

  const plots = Array.from({ length: data.slots }, (_, i) =>
    data.plots.find((p) => p.slot === i + 1),
  );
  const nextRefresh = new Date(data.next_refresh * 1000).toLocaleTimeString(dateLocale(locale), {
    hour: "2-digit",
    minute: "2-digit",
  });
  const refreshIn = now === null ? "" : fmt(tf.refreshIn, { t: leftText(data.next_refresh * 1000 - now) });
  const firstEmpty = plots.findIndex((p) => !p) + 1;

  function plant(cropId: number) {
    const slot = picking ?? firstEmpty;
    if (slot === 0) {
      setErr(tf.full);
      return;
    }
    void act(
      "/farm/plant",
      { slot, crop_id: cropId },
      (d) =>
        fmtCur(
          tf.plantOk,
          {
            crop: (d as unknown as { crop: string }).crop,
            cost: (d as unknown as { cost: number }).cost,
          },
          currency,
        ),
    );
  }

  return (
    <div className="flex flex-col gap-4">
      <div className="flex flex-wrap items-center gap-2">
        <Link
          href="/games"
          className="min-h-[36px] rounded-full border border-line px-3 text-xs font-bold text-sub"
        >
          ← {tg.back}
        </Link>
        <h1 className="font-display text-2xl">🌾 {tf.title.replace("{magic}", currency)}</h1>
        <span className="text-sm text-sub">
          {fmt(tf.marketRule, { time: nextRefresh })} · {refreshIn}
        </span>
      </div>

      <BalanceBar
        balance={me?.balance ?? null}
        todayNet={me?.today_net ?? null}
        limitLeft={me?.limit_left ?? null}
      />

      <section className="rounded-[var(--r-lg)] border border-line bg-[var(--surface-card)] p-4 shadow-[var(--shadow-card)]">
        <div className="mb-3 flex flex-wrap items-center justify-between gap-2">
          <h2 className="font-display text-base">{tf.myField}</h2>
          <span className="text-xs text-sub">
            {picking ? fmt(tf.picking, { n: picking }) : tf.plotHint}
          </span>
        </div>
        <div className="grid grid-cols-2 gap-3 md:grid-cols-3">
          {plots.map((p, i) => (
            <FarmPlot
              key={i}
              slot={i + 1}
              plot={p}
              crop={data.crops.find((c) => c.id === p?.crop_id)}
              now={now}
              busy={busy}
              onPlant={(s) => setPicking(s)}
              onWater={(s) =>
                void act("/farm/water", { slot: s }, () => tf.waterOk)
              }
              onHarvest={(s) =>
                void act(
                  "/farm/harvest",
                  { slot: s },
                  (d) =>
                    fmtCur(
                      tf.harvestOk,
                      {
                        crop: (d as unknown as { crop: string }).crop,
                        amount: (d as unknown as { amount: number }).amount,
                        doubled: (d as unknown as { doubled: boolean }).doubled ? tf.doubled : "",
                      },
                      currency,
                    ),
                  (d) => ((d as unknown as { doubled: boolean }).doubled ? "jackpot" : "win"),
                )
              }
              t={tf as unknown as Record<string, string>}
            />
          ))}
        </div>
      </section>

      <section className="rounded-[var(--r-lg)] border border-line bg-[var(--surface-card)] p-4 shadow-[var(--shadow-card)]">
        <h2 className="mb-3 font-display text-base">{tf.market}</h2>
        <div className="grid grid-cols-2 gap-3 md:grid-cols-3 lg:grid-cols-5">
          {data.crops.map((c) => (
            <MarketCard
              key={c.id}
              crop={c}
              onPlant={plant}
              t={tf as unknown as Record<string, string>}
            />
          ))}
        </div>
      </section>

      <ResultFlash kind={msg?.kind ?? null} text={msg?.text ?? null} />
      {err && <p className="text-xs text-danger">{err}</p>}
    </div>
  );
}

function leftText(ms: number): string {
  if (ms <= 0) return "00:00:00";
  const h = Math.floor(ms / 3600000);
  const m = Math.floor((ms % 3600000) / 60000);
  const s = Math.floor((ms % 60000) / 1000);
  return `${String(h).padStart(2, "0")}:${String(m).padStart(2, "0")}:${String(s).padStart(2, "0")}`;
}
