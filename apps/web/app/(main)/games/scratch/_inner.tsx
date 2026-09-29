"use client";

import { PANEL_LG, PANEL_LG_COL } from "@/lib/ui-classes";

import { useCallback, useEffect, useRef, useState } from "react";
import { api, ApiError } from "@/lib/api-client";
import { useI18n } from "@/i18n/client";
import { fmtCur } from "@/i18n/config";
import { ChipSelect, GameShell, PlayHint } from "@/components/game/game-kit";
import { HistoryStrip, ResultFlash } from "@/components/game/game-kit-feedback";
import { ScratchCanvas } from "@/components/game/scratch-canvas";
import { GameStage } from "@/components/game/game-stage";
import { scratchPoolText, type ScratchPrize } from "@/lib/games";

export interface Overview {
  max_bet: number;
  me?: {
    balance: number;
    today_net: number;
    today_plays: number;
    limit_left: number;
  };
  scratch?: { prizes: ScratchPrize[]; empty_pct: number };
}

interface RoundRow {
  game: string;
  bet: number;
  payout: number;
  net: number;
}

/** 刮刮乐专注页：买卡 → 真刮 → 刮开过半自动开完 → 揭晓 */
export default function ScratchPage({
  initialOver,
}: {
  initialOver: Overview | null;
}) {
  const { dict, currency } = useI18n();
  const t = dict.games;
  const ts = dict.games.scratch;

  const [ov, setOv] = useState<Overview | null>(initialOver);
  const [hist, setHist] = useState<RoundRow[]>([]);
  const [bet, setBet] = useState(100);
  const [phase, setPhase] = useState<
    "idle" | "buying" | "scratchable" | "done"
  >("idle");
  const [outcome, setOutcome] = useState<{
    multiplier: number;
    payout: number;
    net: number;
  } | null>(null);
  const [pct, setPct] = useState(0);
  const [flash, setFlash] = useState<{
    kind: "win" | "lose" | "tie" | "jackpot";
    text: string;
  } | null>(null);
  const [autoReveal, setAutoReveal] = useState(false);
  const [round, setRound] = useState(0);
  const [err, setErr] = useState<string | null>(null);
  const [reduced, setReduced] = useState(false);
  /** 本次会话已完成的局数（防沉迷软提示用） */
  const [sessionPlays, setSessionPlays] = useState(0);
  const idem = useRef<string>("");

  useEffect(() => {
    setReduced(window.matchMedia("(prefers-reduced-motion: reduce)").matches);
  }, []);

  // 切后台 / 切走页面：已买卡就自动开完。
  // 结果在买卡那一刻就由服务端定了，不能让用户回来对着半刮的卡（钱已扣、奖已开）。
  useEffect(() => {
    const onVis = () => {
      if (document.visibilityState === "hidden" && phase === "scratchable")
        setAutoReveal(true);
    };
    document.addEventListener("visibilitychange", onVis);
    return () => document.removeEventListener("visibilitychange", onVis);
  }, [phase]);

  const loadMeta = useCallback(async () => {
    try {
      setOv(await api.get<Overview>("/api/v1/games"));
    } catch {
      /* 顶栏数据取不到不影响玩，缺省用设置上限 */
    }
    try {
      setHist(
        await api.get<RoundRow[]>("/api/v1/games/rounds?game=scratch&limit=10"),
      );
    } catch {
      /* 忽略 */
    }
  }, []);

  useEffect(() => {
    void loadMeta();
  }, [loadMeta]);

  const maxBet = ov?.max_bet ?? 1000;

  async function buy() {
    setErr(null);
    setFlash(null);
    setPct(0);
    setAutoReveal(false);
    setPhase("buying");
    idem.current =
      typeof crypto !== "undefined" && crypto.randomUUID
        ? crypto.randomUUID()
        : String(Date.now());
    try {
      const r = await api.post<{
        multiplier: number;
        payout: number;
        net: number;
      }>("/api/v1/games/scratch", { bet, idempotency_key: idem.current });
      setOutcome(r);
      setRound((n) => n + 1);
      setPhase("scratchable");
      if (reduced) setAutoReveal(true);
    } catch (e) {
      // 1002 是校验类错误：字典里的「参数校验失败」太笼统，直接用后端的具体原因
      setErr(
        e instanceof ApiError
          ? e.code === 1002
            ? e.message
            : (dict.errors[e.code] ?? e.message)
          : dict.common.networkError,
      );
      setPhase("idle");
    }
  }

  function onRevealed() {
    const o = outcome;
    setPhase("done");
    setSessionPlays((n) => n + 1);
    if (!o) return;
    const kind: "win" | "lose" | "tie" | "jackpot" =
      o.multiplier >= 2
        ? "jackpot"
        : o.net > 0
          ? "win"
          : o.net === 0
            ? "tie"
            : "lose";
    setFlash({
      kind,
      text:
        o.multiplier >= 2
          ? fmtCur(ts.jackpot, { mult: o.multiplier, net: o.net }, currency)
          : o.net > 0
            ? fmtCur(ts.win, { mult: o.multiplier, net: o.net }, currency)
            : o.net === 0
              ? ts.breakeven
              : fmtCur(ts.lose, { net: -o.net }, currency),
    });
    void loadMeta();
  }

  const prizeFace = () => {
    if (phase === "buying")
      return <span className="text-sm text-sub">{ts.buying}</span>;
    if (!outcome)
      return <span className="num text-4xl font-black text-sub">??</span>;
    const big = outcome.multiplier >= 2;
    return (
      <div className="flex flex-col items-center">
        <span
          className={`num font-display text-[40px] ${big ? "text-[var(--warning)]" : outcome.multiplier > 0 ? "text-mint" : "text-sub"}`}
        >
          {outcome.multiplier === 0 ? "0" : `${outcome.multiplier}x`}
        </span>
        <span className="text-xs text-sub">
          {outcome.multiplier === 0
            ? ts.thanks
            : `${ts.payout} ${outcome.payout}`}
        </span>
      </div>
    );
  };

  return (
    <GameShell
      icon="🎫"
      title={ts.title}
      subtitle={scratchPoolText(ov?.scratch?.prizes, ts.poolLabel) || ts.pool}
      balance={ov?.me?.balance ?? null}
      todayNet={ov?.me?.today_net ?? null}
      limitLeft={ov?.me?.limit_left ?? null}
      notice={
        <PlayHint
          sessionPlays={sessionPlays}
          todayNet={ov?.me?.today_net ?? null}
        />
      }
      stage={
        <div className="flex w-full flex-col items-center gap-3">
          <GameStage
            art="/games/stage-scratch.jpg"
            className="h-[210px] w-full max-w-[460px]"
          >
            <div className="flex h-full flex-col items-center justify-center">
              {prizeFace()}
            </div>
            <ScratchCanvas
              key={round}
              armed={phase === "scratchable"}
              revealAll={autoReveal}
              onProgress={setPct}
              onRevealed={onRevealed}
              label={ts.coatLabel}
            />
          </GameStage>
          <p className="text-xs text-sub">
            {phase === "idle" && !err
              ? ts.hintIdle
              : phase === "buying"
                ? ts.buying
                : phase === "scratchable"
                  ? ts.scratched.replace("{pct}", String(Math.round(pct * 100)))
                  : ts.hintDone}
          </p>
          <ResultFlash kind={flash?.kind ?? null} text={flash?.text ?? null} />
          {err && <p className="text-xs text-danger">{err}</p>}
        </div>
      }
      controls={
        <div className={PANEL_LG_COL}>
          <ChipSelect
            value={bet}
            onChange={setBet}
            maxBet={maxBet}
            disabled={phase === "buying" || phase === "scratchable"}
          />
          <div className="flex flex-wrap gap-2">
            <button
              type="button"
              onClick={buy}
              disabled={phase === "buying" || phase === "scratchable"}
              className="min-h-[48px] flex-1 rounded-full bg-coral px-6 font-black text-white active:scale-[0.97] disabled:opacity-50"
            >
              {phase === "buying"
                ? ts.buying
                : ts.buy.replace("{n}", String(bet))}
            </button>
            <button
              type="button"
              onClick={() => setAutoReveal(true)}
              disabled={phase !== "scratchable"}
              className="min-h-[48px] rounded-full border border-[var(--border-deep)] px-4 text-sm font-bold disabled:opacity-50"
            >
              {ts.autoScratch}
            </button>
          </div>
        </div>
      }
      side={
        <div className={PANEL_LG}>
          <h2 className="mb-2 font-display text-base">{t.history}</h2>
          <HistoryStrip rounds={hist} />
          <p className="mt-3 text-[11px] text-sub">
            {scratchPoolText(ov?.scratch?.prizes, ts.poolLabel) || ts.pool}
          </p>
        </div>
      }
    />
  );
}
