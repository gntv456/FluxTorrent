"use client";

import { useCallback, useEffect, useRef, useState } from "react";
import Link from "next/link";
import {
  CHIP_POND_BADGE,
  PANEL_LG_COL,
  PANEL_WARN_ORANGE,
  STAGE_STAT_LINE,
} from "@/lib/ui-classes";
import { api, ApiError, isRetryableNetError } from "@/lib/api-client";
import { apiErrorMessage, useI18n } from "@/i18n/client";
import { fmtCur } from "@/i18n/config";
import { ChipSelect, GameShell, PlayHint } from "@/components/game/game-kit";
import { GameTabs } from "@/components/game/game-tabs";
import { GameRecordsPanel } from "@/components/game/game-records";
import { ArcadeGlyph } from "@/components/arcade/arcade-glyph";
import { ResultFlash } from "@/components/game/game-kit-feedback";
import type { FishPhase } from "@/components/game/fishing-pond";
import dynamic from "next/dynamic";

// W-33：重画布（pond 视觉层 ~300 行）延迟加载
const FishingPond = dynamic(() =>
  import("@/components/game/fishing-pond").then((m) => m.FishingPond),
  { loading: () => <SkeletonBlock height={280} /> });
import { FishingSide } from "@/components/game/fishing-side";
import { SwHist, SwStatRow } from "@/components/game/sw-panels";
import { SwRules } from "@/components/game/sw-enrich";
import { clientUuid } from "@/lib/client-uuid";
import { SkeletonBlock } from "@/components/ui/skeleton";

export interface Overview {
  max_bet: number;
  fishing?: {
    ticket: number;
    prizes: {
      label: string; weight_permille: number;
      payout: number; value?: number;
    }[];
  };
  me?: {
    balance: number; today_net: number;
    today_plays: number; limit_left: number;
  };
}

interface RoundRow {
  game: string; bet: number; payout: number; net: number; at?: string;
}

interface CastResult {
  round_id: number;
  bet: number;
  bite_after_ms: number;
  window_ms: number;
}

interface ReelResult {
  won: boolean;
  prize?: string;
  payout: number;
  value?: number;
  net: number;
  bet: number;
  elapsed_ms: number;
  bite_after_ms: number;
}

/** 钓鱼专注页：抛竿 → 咬钩 → 窗口内起竿（计时由服务端判）。 */
export default function FishingPage({
  initialOver,
}: {
  initialOver: Overview | null;
}) {
  const { dict, currency } = useI18n();
  const tf = dict.games.fishing;

  const [ov, setOv] = useState<Overview | null>(initialOver);
  const [hist, setHist] = useState<RoundRow[]>([]);
  const [bet, setBet] = useState(100);
  const [phase, setPhase] = useState<FishPhase>("idle");
  const [round, setRound] = useState<CastResult | null>(null);
  const [windowPct, setWindowPct] = useState(1);
  const [flash, setFlash] = useState<{
    kind: "win" | "lose" | "tie" | "jackpot";
    text: string;
  } | null>(null);
  const [err, setErr] = useState<string | null>(null);
  // P1：网络错误不能伪装成「鱼跑了」的确定失败
  const [reelUnknown, setReelUnknown] = useState(false);
  const [tab, setTab] = useState("play");
  const [reduced, setReduced] = useState(false);
  const [sessionPlays, setSessionPlays] = useState(0);
  const idem = useRef<string>("");
  const biteTimer = useRef<number | null>(null);
  const tickTimer = useRef<number | null>(null);

  useEffect(() => {
    setReduced(window.matchMedia("(prefers-reduced-motion: reduce)").matches);
  }, []);

  const clearTimers = useCallback(() => {
    if (biteTimer.current) window.clearTimeout(biteTimer.current);
    if (tickTimer.current) window.clearInterval(tickTimer.current);
    biteTimer.current = null;
    tickTimer.current = null;
  }, []);

  useEffect(() => () => clearTimers(), [clearTimers]);

  const loadMeta = useCallback(async () => {
    const [ov, rounds] = await Promise.all([
      api.get<Overview>("/api/v1/games").catch(() => null),
      api
        .get<RoundRow[] | { rows: RoundRow[] }>(
          "/api/v1/games/rounds?game=fishing&limit=10",
        )
        .catch(() => null),
    ]);
    if (ov) setOv(ov);
    setHist(Array.isArray(rounds) ? rounds : (rounds?.rows ?? []));
  }, []);

  useEffect(() => {
    void loadMeta();
  }, [loadMeta]);

  const ticket = ov?.fishing?.ticket ?? 100;
  const prizes = ov?.fishing?.prizes ?? [];
  const winRate = prizes.length
    ? (100 - (prizes[0]?.weight_permille ?? 0) / 10).toFixed(1)
    : "—";

  /** 起竿：时机由服务端判，前端只提交。
   *  `r0` 显式传入而非读 state：定时器回调捕获的是当轮渲染的旧闭包，
   *  `setRound` 还没反映进去 → 读 state 得 null，自动判负会静默失效。 */
  const reel = useCallback(
    async (r0: CastResult | null, auto: boolean) => {
      clearTimers();
      if (!r0) return;
      setPhase("reeling");
      try {
        const r = await api.post<ReelResult>("/api/v1/games/fishing/reel", {
          round_id: r0.round_id,
        });
        const early = r.elapsed_ms < r.bite_after_ms;
        const won = r.won && (r.value ?? r.payout) > 0;
        const jackpot = (r.value ?? 0) >= r0.bet * 8;
        const kind = jackpot ? "jackpot" : won ? "win" : "lose";
        const bait = String(r0.bet);
        const text = won
          ? fmtCur(
              jackpot ? tf.jackpot : tf.win,
              { prize: r.prize ?? "", net: r.net },
              currency,
            )
          : r.won
            ? tf.empty.replace("{n}", bait).replace("{magic}", currency)
            : (early ? tf.early : tf.missed)
                .replace("{n}", bait)
                .replace("{magic}", currency);
        setFlash({ kind, text });
        setSessionPlays((n) => n + 1);
      } catch (e) {
        setErr(
          e instanceof ApiError
            ? e.code === 1002
              ? e.message
              : apiErrorMessage(dict, e)
            : dict.common.networkError,
        );
        // 网络错误 ≠ 玩家失误：此前 auto 模式直接判负「鱼跑了」，
        // 把服务端故障伪装成确定性失败。改为「结果未知」+ 查询入口。
        if (isRetryableNetError(e)) {
          setReelUnknown(true);
        }
      } finally {
        setRound(null);
        setPhase("done");
        void loadMeta();
      }
    },
    [clearTimers, tf, currency, dict],
  );

  async function cast() {
    if (phase === "casting" || phase === "waiting" || phase === "reeling")
      return;
    setErr(null);
    setFlash(null);
    setRound(null);
    setPhase("casting");
    idem.current =
      clientUuid();
    try {
      const r = await api.post<CastResult>("/api/v1/games/fishing/cast", {
        bet,
        idempotency_key: idem.current,
      });
      setRound(r);
      setWindowPct(1);
      setPhase("waiting");
      if (reduced) {
        // 减少动效：直接进入可起竿态。窗口计时不能省——reduced 玩家
        // 也要有真实的窗口递减 + 超时自动判负（否则满格条说谎）
        setPhase("bite");
        const start = performance.now();
        tickTimer.current = window.setInterval(() => {
          const left = 1 - (performance.now() - start) / r.window_ms;
          setWindowPct(Math.max(0, left));
          if (left <= 0) {
            window.clearInterval(tickTimer.current ?? undefined);
            void reel(r, true);
          }
        }, 100);
        return;
      }
      biteTimer.current = window.setTimeout(() => {
        setPhase("bite");
        const start = performance.now();
        tickTimer.current = window.setInterval(() => {
          const left = 1 - (performance.now() - start) / r.window_ms;
          setWindowPct(Math.max(0, left));
          if (left <= 0) {
            window.clearInterval(tickTimer.current ?? undefined);
            void reel(r, true);
          }
        }, 50);
      }, r.bite_after_ms);
    } catch (e) {
      setErr(
        e instanceof ApiError
          ? e.code === 1002
            ? e.message
            : (apiErrorMessage(dict, e))
          : dict.common.networkError,
      );
      setPhase("idle");
    }
  }

  const canCast = phase === "idle" || phase === "done";
  const canReel = phase === "bite" || phase === "waiting";

  if (tab === "records") {
    return (
      <div className="flex flex-col gap-4">
        <GameTabs
          tabs={[
            { key: "play", label: dict.games.sub.tabPlay, icon: "🎣" },
            { key: "records", label: dict.games.sub.tabRecords, icon: "📜" },
          ]}
          active={tab}
          onChange={setTab}
        />
        <GameRecordsPanel game="fishing" />
      </div>
    );
  }
  return (
    <>
      <GameShell
      icon={<ArcadeGlyph k="fishing" />}
      title={tf.title}
      subtitle={`${tf.winRate.replace("{pct}", winRate)} · ${tf.sub}`}
      balance={ov?.me?.balance ?? null}
      todayNet={ov?.me?.today_net ?? null}
      limitLeft={ov?.me?.limit_left ?? null}
      /* 布局优化（2026-11-23）：标签条从 notice 槽（独占一行）挪进
         tabBar 槽与余额条同行；「完整渔场」入口从舞台上方 56px 横幅
         收成同行小按钮。头部各省一行。 */
      tabBar={
        <>
          <GameTabs
            tabs={[
              { key: "play", label: dict.games.sub.tabPlay, icon: "🎣" },
              {
                key: "records",
                label: dict.games.sub.tabRecords,
                icon: "📜",
              },
            ]}
            active={tab}
            onChange={setTab}
          />
          <Link
            href="/games/fishing/pond"
            title={`${tf.pondTitle} · ${tf.pondSub}`}
            className="arc-chip ml-auto whitespace-nowrap"
          >
            {tf.pondCrumb} →
          </Link>
        </>
      }
      notice={
        <PlayHint
          sessionPlays={sessionPlays}
          todayNet={ov?.me?.today_net ?? null}
        />
      }
      stage={
        <div className="flex w-full flex-col items-center gap-3">
          {/* 样图⑩：全出血星夜海（GameStage 的内框样式被甜梦层
              撤掉——fp-pond 直接铺满宽、占首屏 ~60%） */}
          <FishingPond
            phase={phase}
            windowPct={windowPct}
            waitingLabel={phase === "idle" ? tf.hintIdle : tf.waiting}
            biteLabel={tf.bite}
          />
          <ResultFlash kind={flash?.kind ?? null} text={flash?.text ?? null} />
          {/* 布局优化（2026-11-23）：今日三格统计压成一行细条随舞台；
              3 行历史 + 规则卡 + 渔场横幅全部移出舞台（历史在侧栏
              「战绩」标签，规则在「奖池」标签底部），舞台只剩池塘本体。 */}
          <div className={STAGE_STAT_LINE}>
            <span>
              {tf.fTodayCount}{" "}
              <b className="num text-ink">
                {hist.filter((h) => h.net > 0).length}
              </b>
            </span>
            <span>
              {tf.fTodayGold}{" "}
              <b className="num text-[var(--warning)]">
                {hist.filter((h) => h.net >= (ticket ?? 100) * 5).length}
              </b>
            </span>
            <span>
              {tf.fBest}{" "}
              <b className="num text-[var(--mint)]">
                {hist.length > 0
                  ? `+${Math.max(...hist.map((h) => h.net))}`
                  : "—"}
              </b>
            </span>
          </div>
          {err && <p className="text-xs text-[var(--danger-text)]">{err}</p>}
          {reelUnknown && (
            <div className={PANEL_WARN_ORANGE} role="status">
              <p>{dict.review.resultUnknownBody}</p>
              <a
                href="/games/fishing/records"
                className="mt-1 inline-block min-h-11 py-1 text-link underline"
              >
                {dict.review.checkResult} →
              </a>
            </div>
          )}
        </div>
      }
      controls={
        <div className={PANEL_LG_COL}>
          <ChipSelect
            value={bet}
            onChange={setBet}
            maxBet={ov?.max_bet ?? 1000}
            disabled={!canCast && phase !== "bite"}
          />
          {/* CTA 双按钮（样图⑩）：抛竿（蓝）/ 起竿（珊瑚）；
              咬钩态切换主行动，饵量文案并入按钮下 */}
          <div className="sw-cta-row">
            {canReel ? (
              <button
                type="button"
                onClick={() => reel(round, false)}
                className="sw-cta cta-gold"
              >
                {tf.reel}
              </button>
            ) : (
              <button
                type="button"
                onClick={cast}
                disabled={!canCast || prizes.length === 0}
                className="sw-cta cta-primary"
              >
                {phase === "casting"
                  ? tf.casting
                  : phase === "reeling"
                    ? tf.reeling
                    : tf.cast}
              </button>
            )}
          </div>
          <p className="text-xs text-sub">
            {tf.bait.replace("{n}", String(bet))} · {tf.hintIdle}
          </p>
        </div>
      }
      side={<FishingSide prizes={prizes} ticket={ticket} hist={hist} />}
    />
    </>
  );
}
