"use client";

import { useCallback, useEffect, useState } from "react";
import { api, ApiError } from "@/lib/api-client";
import { useI18n } from "@/i18n/client";
import { dateLocale, fmt, fmtCur } from "@/i18n/config";
import { BalanceBar } from "@/components/game/game-kit";
import { GameToast } from "@/components/game/game-kit-feedback";
import { MarketSection, MyFieldSection } from "./_inner-sections";
import Link from "next/link";

export interface FarmData {
  window_start: number;
  next_refresh: number;
  crops: import("@/components/game/farm-field").Crop[];
  plots: import("@/components/game/farm-field").Plot[];
  slots: number;
  /** 农场自己的限流配额（rl:farm，与即时赌局分开计数） */
  hour_limit?: number;
  hour_left?: number;
  /** 作物有效期（天，0 = 永不枯萎） */
  wither_days?: number;
}

export interface MeData {
  balance: number;
  today_net: number;
  today_plays: number;
  limit_left: number;
}

/** 农场专注页：六块田 + 图鉴式行情（旧 /farm 已重定向到此）。
 *  我的田地与行情两个分区拆到 _inner-sections.tsx */
export default function FarmPage({
  initialFarm,
  initialMe,
}: {
  initialFarm: FarmData | null;
  initialMe: MeData | null;
}) {
  const { dict, locale, currency } = useI18n();
  const tf = dict.farm;
  const tg = dict.games;

  const [data, setData] = useState<FarmData | null>(initialFarm);
  const [me, setMe] = useState<MeData | null>(initialMe);
  const [now, setNow] = useState<number | null>(null);
  const [msg, setMsg] = useState<{
    kind: "win" | "lose" | "tie" | "jackpot";
    text: string;
  } | null>(null);
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
      const r = await api.get<{ me?: MeData }>("/api/v1/games");
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

  // 市场窗口到点**自动刷新**（行情价与地块状态一起更新，不用手动 F5）
  useEffect(() => {
    if (!data) return;
    const ms = data.next_refresh * 1000 - Date.now() + 1500;
    const id = window.setTimeout(
      () => {
        void refresh();
        void loadMe();
      },
      Math.max(1000, ms),
    );
    return () => window.clearTimeout(id);
  }, [data, refresh, loadMe]);

  async function act(
    path: string,
    body: unknown,
    ok: (d: never) => string,
    kindOf?: (d: never) => "win" | "lose" | "jackpot",
  ): Promise<void> {
    setBusy(true);
    setErr(null);
    setMsg(null);
    try {
      const r = await api.post<Record<string, never>>(`/api/v1${path}`, body);
      setMsg({
        kind: kindOf ? kindOf(r as never) : "win",
        text: ok(r as never),
      });
      setPicking(null);
      await refresh();
      void loadMe();
    } catch (e) {
      // 1002 用后端具体原因（「该地块已有作物」「尚未成熟」等）
      setErr(
        e instanceof ApiError
          ? e.code === 1002
            ? e.message
            : (dict.errors[e.code] ?? e.message)
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
  // 时间文案一律等客户端挂载后再算：容器时区（UTC）与浏览器时区不同，
  // 在 SSR 阶段格式化绝对时间会造成 hydration 文本不匹配（React #418）。
  const nextRefresh =
    now === null
      ? ""
      : new Date(data.next_refresh * 1000).toLocaleTimeString(
          dateLocale(locale),
          {
            hour: "2-digit",
            minute: "2-digit",
          },
        );
  const refreshIn =
    now === null
      ? ""
      : fmt(tf.refreshIn, { t: leftText(data.next_refresh * 1000 - now) });
  const firstEmpty = plots.findIndex((p) => !p) + 1;
  const readyPlots = plots.filter((p) => p && p.ready && !p.withered);

  /** 一键收获：逐块调同一 harvest 端点（后端幂等 + 行锁，重复点也安全）；
   *  单块失败不中断其余（例如某块刚好被并发收走）。 */
  async function harvestAll() {
    if (busy || readyPlots.length === 0) return;
    setBusy(true);
    setErr(null);
    setMsg(null);
    let total = 0;
    let doubled = 0;
    let failed = 0;
    for (const p of readyPlots) {
      try {
        const r = await api.post<{ amount: number; doubled: boolean }>(
          "/api/v1/farm/harvest",
          {
            slot: p!.slot,
          },
        );
        total += r.amount;
        if (r.doubled) doubled++;
      } catch {
        failed++;
      }
    }
    setMsg({
      kind: doubled > 0 ? "jackpot" : "win",
      text: `${fmtCur(tf.harvestAllOk, { n: total, c: readyPlots.length - failed }, currency)}${
        doubled > 0 ? ` ${tf.doubled}` : ""
      }${failed > 0 ? ` · ${tf.harvestAllFail.replace("{n}", String(failed))}` : ""}`,
    });
    setBusy(false);
    await refresh();
    void loadMe();
  }

  function plant(cropId: number) {
    const slot = picking ?? firstEmpty;
    if (slot === 0) {
      setErr(tf.full);
      return;
    }
    void act("/farm/plant", { slot, crop_id: cropId }, (d) =>
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
        <h1 className="font-display text-2xl">
          🌾 {tf.title.replace("{magic}", currency)}
        </h1>
        <span className="text-sm text-sub">
          {fmt(tf.marketRule, { time: nextRefresh })}
          {refreshIn && ` · ${refreshIn}`}
        </span>
      </div>

      <BalanceBar
        balance={me?.balance ?? null}
        todayNet={me?.today_net ?? null}
        limitLeft={data.hour_left ?? null}
        limitText={
          data.hour_left === undefined
            ? undefined
            : tf.hourLeft.replace("{n}", String(data.hour_left))
        }
      />

      {/* 错误放在顶部（长页面里底部提示会跑出视野） */}
      {err && (
        <p className="rounded-[var(--r-md)] bg-coral-soft px-3 py-2 text-xs font-bold text-ink">
          {err}
        </p>
      )}

      <MyFieldSection
        data={data}
        plots={plots}
        now={now}
        busy={busy}
        picking={picking}
        setPicking={setPicking}
        readyPlots={readyPlots}
        tf={tf as unknown as Record<string, string>}
        currency={currency}
        onHarvestAll={harvestAll}
        act={act}
      />

      <MarketSection
        crops={data.crops}
        onPlant={plant}
        tf={tf as unknown as Record<string, string>}
      />

      {/* 动作反馈走浮层：田地与行情都很长，固定在视口下方的提示不会跑出视野 */}
      <GameToast message={msg} onDone={() => setMsg(null)} />
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
