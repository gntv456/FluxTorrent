"use client";

import { useCallback, useEffect, useState } from "react";
import { api, ApiError } from "@/lib/api-client";
import { useI18n } from "@/i18n/client";
import { dateLocale, fmt, fmtCur } from "@/i18n/config";
import { countdownText, eggText, type EggPrize } from "@/lib/games";
import { ArcadeGlyph } from "@/components/arcade/arcade-glyph";
import type { FarmLand } from "@/components/game/farm-land";
import { GameTabs } from "@/components/game/game-tabs";
import { BalanceBar } from "@/components/game/game-kit";
import { GameToast } from "@/components/game/game-kit-feedback";
import { CraftSection, FarmAlbumSection, RanchSection } from "./_inner-ranch";
import {
  MarketSection,
  MyFieldSection,
  FarmQuestSection,
} from "./_inner-sections";
import Link from "next/link";

export interface FarmData {
  window_start: number;
  next_refresh: number;
  crops: import("@/components/game/farm-field").Crop[];
  plots: import("@/components/game/farm-field").Plot[];
  slots: number;
  /** 土地阶梯（买地/升级）；旧响应没这块时按免费地块数画田 */
  land?: FarmLand;
  /** 市场价刷新口径（服务端按设置键算出的文字，前端不抄第二份） */
  market_refresh?: string;
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

type ToastMsg = { kind: "win" | "lose" | "tie" | "jackpot"; text: string };

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
  const [msg, setMsg] = useState<ToastMsg | null>(null);
  const [err, setErr] = useState<string | null>(null);
  const [busy, setBusy] = useState(false);
  const [picking, setPicking] = useState<number | null>(null);

  // 农场周常任务（样图⑤任务卡）：arcade-meta 的 quests 里筛 farm 系行。
  // 大厅组件 ArcadeMeta 也会拉同一端点，但它是另一棵树 —— 这里独立拉取。
  const [questPack, setQuestPack] = useState<{
    period: string;
    items: {
      code: string;
      done: number;
      target: number;
      claimed: boolean;
      ready: boolean;
      reward: number;
      item_name?: string | null;
    }[];
  } | null>(null);
  const [claiming, setClaiming] = useState<string | null>(null);
  // 样图⑤四 Tab：田园 / 牧场 / 加工 / 图鉴（区块按 tab 切换，不平铺）
  const [tab, setTab] = useState("field");

  const loadQuests = useCallback(async () => {
    try {
      const r = await api.get<{
        quests: {
          period: string;
          items: {
            code: string;
            done: number;
            target: number;
            claimed: boolean;
            ready: boolean;
            reward: number;
            item_name?: string | null;
          }[];
        };
      }>("/api/v1/games/arcade-meta");
      setQuestPack(r.quests);
    } catch {
      /* 未配置/关闭时静默：任务卡整体不渲染 */
    }
  }, []);

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
    void loadQuests();
  }, [refresh, loadMe, loadQuests]);

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
    // 双击防抖：与其它玩法的 draw() 同纪律（市场行播种按钮共用此入口）
    if (busy) return;
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

  if (!data)
    return (
      <div className="py-16 text-center text-sub">{err ?? tf.loading}</div>
    );

  // 田里有几块地由土地阶梯定（买来的排在免费地后面）；旧响应没带 land 时退回免费数
  const owned = data.land?.owned ?? data.slots;
  const plots = Array.from({ length: owned }, (_, i) =>
    data.plots.find((p) => p.slot === i + 1),
  );
  // 时间文案一律等客户端挂载后再算：容器时区（UTC）与浏览器时区不同，
  // 在 SSR 阶段格式化绝对时间会造成 hydration 文本不匹配（React #418）。
  const nextRefresh =
    now === null
      ? ""
      : new Date(data.next_refresh * 1000).toLocaleTimeString(
          dateLocale(locale),
          { hour: "2-digit", minute: "2-digit" },
        );
  const refreshIn =
    now === null
      ? ""
      : fmt(tf.refreshIn, { t: countdownText(data.next_refresh * 1000 - now) });
  const firstEmpty = plots.findIndex((p) => !p) + 1;
  const readyPlots = plots.filter((p) => p && p.ready && !p.withered);
  // 行情三格（样图⑤）：标签已由 sw-stat-lb 渲染，值用裸数字避免
  // 「已种 / 已种 0/6」式重复；带标签的完整句式（fsPlanted/fsReady）
  // 只给农情条和「我的田」strip 用
  const fsPlantedLabel = tf.fsPlantedLabel;
  const fsReadyLabel = tf.fsReadyLabel;
  const planted = plots.filter((p) => p).length;
  const fsPriceText = (() => {
    const rows = data.crops.filter((c) => c.active && c.market_price > 0);
    if (rows.length === 0) return "—";
    const ups = rows.filter((c) => c.market_price > c.seed_price).length;
    return `+${Math.round((ups / rows.length) * 100)}%`;
  })();
  const fsPlantedText = `${planted}/${owned}`;
  const fsReadyText = tf.fsReadyNum.replace("{n}", String(readyPlots.length));

  /** 农场任务行的展示文案：大厅 arcade 段按 code 给了每条任务的措辞，
   *  这里沿用同一来源（q_farm / q_farm_fert …），没有词条的任务退
   *  game_ref 原文——不造假数据。 */
  const ta = dict.games.arcade as Record<string, string>;
  const farmQuests = (questPack?.items ?? [])
    .filter((q) => q.code.startsWith("q_farm"))
    .map((q) => ({
      ...q,
      label: ta[q.code] ?? q.code,
    }));
  const questPeriod = questPack?.period ?? "";

  /** 领取农场周常：与大厅 ArcadeMeta 同一端点；领取后回读任务面与余额 */
  async function claimQuest(code: string) {
    if (claiming) return;
    setClaiming(`quest:${code}`);
    setErr(null);
    try {
      await api.post(`/api/v1/games/arcade/quest/${code}/claim`, {
        idempotency_key: `farm-q-${code}-${Date.now()}`,
      });
      setMsg({ kind: "win", text: tf.questClaimed ?? "✓ 已领取" });
      await loadQuests();
      void loadMe();
    } catch (e) {
      setErr(
        e instanceof ApiError
          ? (dict.errors[e.code] ?? e.message)
          : dict.common.networkError,
      );
    } finally {
      setClaiming(null);
    }
  }

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
    let eggs = 0;
    for (const p of readyPlots) {
      try {
        const r = await api.post<{
          amount: number;
          doubled: boolean;
          prize?: EggPrize;
        }>("/api/v1/farm/harvest", {
          slot: p!.slot,
        });
        total += r.amount;
        if (r.doubled) doubled++;
        // 「这一档彩蛋响不响」与单块收获共用同一份判断：空档不出声
        if (eggText(tf, r.prize)) eggs++;
      } catch {
        failed++;
      }
    }
    setMsg({
      kind: doubled > 0 || eggs > 0 ? "jackpot" : "win",
      text: `${fmtCur(tf.harvestAllOk, { n: total, c: readyPlots.length - failed }, currency)}${
        doubled > 0 ? ` ${tf.doubled}` : ""
      }${failed > 0 ? ` · ${tf.harvestAllFail.replace("{n}", String(failed))}` : ""}${
        eggs > 0 ? fmt(tf.eggSome, { n: eggs }) : ""
      }`,
    });
    setBusy(false);
    await refresh();
    void loadMe();
  }

  function plant(cropId: number) {
    const slot = picking ?? firstEmpty;
    if (slot === 0) {
      setErr(fmt(tf.full, { n: plots.length }));
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
          <span aria-hidden className="gs-title-ic">
            <ArcadeGlyph k="farm" />
          </span>{" "}
          {tf.title.replace("{magic}", currency)}
        </h1>
        <span className="text-sm text-sub">
          {fmt(tf.marketRule, {
            time: nextRefresh,
            rule: data.market_refresh ?? "",
          })}
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

      {/* 样图⑤：田园/牧场/加工/图鉴四 Tab（不平铺） */}
      <GameTabs
        tabs={[
          { key: "field", label: tf.tabField, icon: "🌾" },
          { key: "ranch", label: tf.tabRanch, icon: "🐄" },
          { key: "craft", label: tf.tabCraft, icon: "🏭" },
          { key: "album", label: tf.tabAlbum, icon: "📖" },
        ]}
        active={tab}
        onChange={setTab}
      />

      {/* 错误放在顶部（长页面里底部提示会跑出视野） */}
      {err && (
        <p className="rounded-[var(--r-md)] bg-coral-soft px-3 py-2 text-xs font-bold text-ink">
          {err}
        </p>
      )}

      {/* 农情条（样图⑤天气条的落地版）：站点没有天气系统，文案用
          真实机制——浇水提前 10 分钟成熟；右侧行情刷新倒计时 */}
      <div className="sw-farm-weather">
        <span aria-hidden>💧</span>
        <span>
          <b>{tf.wxSun}</b> · {tf.wxBoostReal}
        </span>
        <span className="wx-right">
          {fmt(tf.marketRule, {
            time: nextRefresh,
            rule: data.market_refresh ?? "",
          })}
        </span>
      </div>

      {/* 行情三格（样图⑤ farm-stats）：菜价 / 已种 / 成熟 */}
      <div className="sw-stat-row sw-farm-stats">
        <div className="sw-stat">
          <div className="sw-stat-lb">{tf.fsPrice}</div>
          <div className="sw-stat-vl num gold">{fsPriceText}</div>
        </div>
        <div className="sw-stat">
          <div className="sw-stat-lb">{fsPlantedLabel}</div>
          <div className="sw-stat-vl num">{fsPlantedText}</div>
        </div>
        <div className="sw-stat">
          <div className="sw-stat-lb">{fsReadyLabel}</div>
          <div className="sw-stat-vl num green">{fsReadyText}</div>
        </div>
      </div>

      {tab === "field" && (
        <>
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

          {/* 农场周常任务（样图⑤任务卡）：数据来自 arcade-meta（_inner 顶层
              拉取），筛 farm 系任务行；领取后 reload quests + 刷余额 */}
          <FarmQuestSection
            quests={farmQuests}
            period={questPeriod}
            tf={tf as unknown as Record<string, string>}
            claiming={claiming}
            onClaim={claimQuest}
          />

          <MarketSection
            crops={data.crops}
            onPlant={plant}
            busy={busy}
            tf={tf as unknown as Record<string, string>}
          />
        </>
      )}

      {/* 牧场 / 加工坊 / 图鉴（样图⑤ 2026-10 补页批）：按 tab 切换，
          各自取数、模块未配置时整段隐藏 */}
      {tab === "ranch" && (
        <RanchSection
          tf={tf as unknown as Record<string, string>}
          onErr={setErr}
        />
      )}
      {tab === "craft" && (
        <CraftSection
          tf={tf as unknown as Record<string, string>}
          onErr={setErr}
        />
      )}
      {tab === "album" && (
        <FarmAlbumSection tf={tf as unknown as Record<string, string>} />
      )}

      {/* 动作反馈走浮层：田地与行情都很长，固定在视口下方的提示不会跑出视野 */}
      <GameToast message={msg} onDone={() => setMsg(null)} />
    </div>
  );
}
