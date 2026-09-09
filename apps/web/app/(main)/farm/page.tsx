"use client";

import { useCallback, useEffect, useState } from "react";
import { api, ApiError } from "@/lib/api-client";
import { useI18n } from "@/i18n/client";
import { dateLocale, fmt } from "@/i18n/config";

interface Crop {
  id: number;
  name: string;
  seed_price: number;
  base_yield: number;
  grow_hours: number;
  market_price: number;
}

interface Plot {
  slot: number;
  crop_id: number;
  crop_name: string;
  planted_at: string;
  ready_at: string;
  watered: boolean;
  ready: boolean;
}

interface FarmData {
  window_start: number;
  next_refresh: number;
  crops: Crop[];
  plots: Plot[];
  slots: number;
}

/** 好学农场（M24 magic_fram 口径）：市场价每日 6 刷 ±50%，20% 双倍收获 */
export default function FarmPage() {
  const { dict, locale } = useI18n();
  const [data, setData] = useState<FarmData | null>(null);
  const [msg, setMsg] = useState<string | null>(null);
  const [busy, setBusy] = useState(false);

  const refresh = useCallback(async () => {
    try {
      setData(await api.get<FarmData>("/api/v1/farm"));
    } catch (e) {
      setMsg(
        e instanceof ApiError && e.code === 2001
          ? dict.errors[2001]
          : dict.common.loadFailed,
      );
    }
  }, [dict]);

  useEffect(() => {
    refresh();
  }, [refresh]);

  async function act(path: string, body: unknown, ok: (d: never) => string) {
    setBusy(true);
    setMsg(null);
    try {
      const r = await api.post<Record<string, never>>(`/api/v1${path}`, body);
      setMsg(ok(r as never));
      refresh();
    } catch (e) {
      setMsg(
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
      <div className="py-16 text-center text-sub">
        {msg ?? dict.farm.loading}
      </div>
    );
  }

  const plots = Array.from({ length: data.slots }, (_, i) =>
    data.plots.find((p) => p.slot === i + 1),
  );
  const nextRefresh = new Date(data.next_refresh * 1000).toLocaleTimeString(
    dateLocale(locale),
    { hour: "2-digit", minute: "2-digit" },
  );

  const fmtReady = (iso: string) => {
    const ms = new Date(iso).getTime() - Date.now();
    if (ms <= 0) return dict.farm.mature;
    const h = Math.floor(ms / 3600000);
    const m = Math.floor((ms % 3600000) / 60000);
    return h > 0
      ? fmt(dict.farm.hours, { h, m })
      : fmt(dict.farm.minutes, { m });
  };

  return (
    <div className="flex flex-col gap-4">
      <div className="flex flex-wrap items-baseline gap-2">
        <h1 className="font-display text-2xl">{dict.farm.title}</h1>
        <span className="text-sm text-sub">
          {fmt(dict.farm.marketRule, { time: nextRefresh })}
        </span>
      </div>

      {/* 我的 6 块地 */}
      <section className="grid grid-cols-2 gap-3 md:grid-cols-3">
        {plots.map((p, i) => (
          <div
            key={i}
            className="flex min-h-[120px] flex-col justify-between rounded-[var(--r-md)] border border-line bg-white p-3 shadow-[var(--shadow-card)]"
          >
            {p ? (
              <>
                <div>
                  <p className="font-bold">
                    {p.crop_name}{" "}
                    {p.ready && (
                      <span className="text-xs text-mint">
                        ✓ {dict.farm.matureShort}
                      </span>
                    )}
                  </p>
                  <p className="mt-1 text-xs text-sub">
                    {p.ready
                      ? dict.farm.ready
                      : fmt(dict.farm.readyIn, { t: fmtReady(p.ready_at) })}
                    {p.watered ? ` · ${dict.farm.watered}` : ""}
                  </p>
                </div>
                <div className="mt-2 flex gap-2">
                  {!p.watered && !p.ready && (
                    <button
                      onClick={() =>
                        act("/farm/water", { slot: p.slot }, () => dict.farm.waterOk)
                      }
                      disabled={busy}
                      className="min-h-[36px] flex-1 rounded-full bg-sky-soft px-3 text-xs font-bold text-ink active:scale-[0.97] disabled:opacity-50"
                    >
                      {dict.farm.water}
                    </button>
                  )}
                  <button
                    onClick={() =>
                      act("/farm/harvest", { slot: p.slot }, (d) =>
                        fmt(dict.farm.harvestOk, {
                          crop: (d as unknown as { crop: string }).crop,
                          amount: (d as unknown as { amount: number }).amount,
                          doubled: (d as unknown as { doubled: boolean }).doubled
                            ? dict.farm.doubled
                            : "",
                        }),
                      )
                    }
                    disabled={busy || !p.ready}
                    className="min-h-[36px] flex-1 rounded-full bg-mint px-3 text-xs font-bold text-white active:scale-[0.97] disabled:opacity-40"
                  >
                    {p.ready ? dict.farm.harvest : dict.farm.notReady}
                  </button>
                </div>
              </>
            ) : (
              <>
                <p className="text-sm text-sub">
                  {fmt(dict.farm.plotEmpty, { n: i + 1 })}
                </p>
                <p className="mt-1 text-[11px] text-sub">{dict.farm.plotHint}</p>
              </>
            )}
          </div>
        ))}
      </section>

      {/* 行情 */}
      <section className="rounded-[var(--r-lg)] border border-line bg-white p-4 shadow-[var(--shadow-card)]">
        <h2 className="mb-3 font-display text-lg">{dict.farm.market}</h2>
        <ul className="flex flex-col divide-y divide-line">
          {data.crops.map((c) => {
            const pct = Math.round((c.market_price / c.seed_price - 1) * 100);
            return (
              <li key={c.id} className="flex items-center gap-3 py-2">
                <div className="flex-1">
                  <p className="text-sm font-bold">{c.name}</p>
                  <p className="text-xs text-sub">
                    {fmt(dict.farm.growInfo, {
                      h: c.grow_hours,
                      p: c.seed_price,
                    })}
                  </p>
                </div>
                <div className="text-right">
                  <p className="num text-sm">{c.market_price}</p>
                  <p
                    className={`text-[11px] ${pct >= 0 ? "text-danger" : "text-mint"}`}
                  >
                    {pct >= 0 ? "+" : ""}
                    {pct}%
                  </p>
                </div>
                <button
                  onClick={() => {
                    const emptySlot = plots.findIndex((p) => !p) + 1;
                    if (emptySlot === 0) {
                      setMsg(dict.farm.full);
                      return;
                    }
                    act("/farm/plant", { slot: emptySlot, crop_id: c.id }, (d) =>
                      fmt(dict.farm.plantOk, {
                        crop: (d as unknown as { crop: string }).crop,
                        cost: (d as unknown as { cost: number }).cost,
                      }),
                    );
                  }}
                  disabled={busy}
                  className="min-h-[36px] rounded-full bg-sun px-4 text-xs font-bold text-ink active:scale-[0.97] disabled:opacity-50"
                >
                  {dict.farm.plant}
                </button>
              </li>
            );
          })}
        </ul>
      </section>

      {msg && (
        <p className="rounded-[var(--r-md)] bg-sky-soft p-3 text-sm text-ink">
          {msg}
        </p>
      )}
    </div>
  );
}
