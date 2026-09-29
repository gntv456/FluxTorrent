"use client";

import { PANEL_LG, PANEL_LG_COL } from "@/lib/ui-classes";

import { useCallback, useEffect, useRef, useState } from "react";
import { api, ApiError } from "@/lib/api-client";
import { useI18n } from "@/i18n/client";
import { fmtCur } from "@/i18n/config";
import { ChipSelect, GameShell, PlayHint } from "@/components/game/game-kit";
import { HistoryStrip, ResultFlash } from "@/components/game/game-kit-feedback";
import { RunwayOdometer } from "@/components/game/runway";
import { GameStage } from "@/components/game/game-stage";

export interface Overview {
  max_bet: number;
  bigsmall?: { win_mult: number; expected_value: number };
  me?: {
    balance: number;
    today_net: number;
    today_plays: number;
    limit_left: number;
  };
}

interface RoundRow {
  game: string;
  bet: number;
  payout: number;
  net: number;
}

interface GuessResult {
  number: number;
  player_win: boolean;
  payout: number;
  net: number;
  tie: boolean;
}

const SPIN_MS = 1750;

/** 猜大小专注页：跑道减速定格 + 路单 */
export default function BigSmallPage({
  initialOver,
}: {
  initialOver: Overview | null;
}) {
  const { dict, currency } = useI18n();
  const t = dict.games;
  const tg = dict.games.bigsmall;

  const [ov, setOv] = useState<Overview | null>(initialOver);
  const [hist, setHist] = useState<RoundRow[]>([]);
  const [bet, setBet] = useState(100);
  const [busy, setBusy] = useState(false);
  const [num, setNum] = useState<number | null>(null);
  const [res, setRes] = useState<GuessResult | null>(null);
  const [flash, setFlash] = useState<{
    kind: "win" | "lose" | "tie";
    text: string;
  } | null>(null);
  const [streak, setStreak] = useState(0);
  /** 本次会话已完成的局数（防沉迷软提示用） */
  const [sessionPlays, setSessionPlays] = useState(0);
  const [err, setErr] = useState<string | null>(null);
  const [reduced, setReduced] = useState(false);
  const idem = useRef<string>("");
  const timer = useRef<number | null>(null);

  useEffect(() => {
    setReduced(window.matchMedia("(prefers-reduced-motion: reduce)").matches);
  }, []);

  const loadMeta = useCallback(async () => {
    try {
      setOv(await api.get<Overview>("/api/v1/games"));
    } catch {
      /* ignore */
    }
    try {
      setHist(
        await api.get<RoundRow[]>(
          "/api/v1/games/rounds?game=bigsmall&limit=20",
        ),
      );
    } catch {
      /* ignore */
    }
  }, []);

  useEffect(() => {
    void loadMeta();
  }, [loadMeta]);

  useEffect(
    () => () => {
      if (timer.current) window.clearTimeout(timer.current);
    },
    [],
  );

  const maxBet = ov?.max_bet ?? 1000;
  // 赔率由后端下发（< 2.0，回收口径），不在前端写死
  const winMult = ov?.bigsmall?.win_mult ?? 1.9;
  // 近 N 局统计（按局聚合的战绩算，不含流水噪音）
  const stats = hist.reduce(
    (a, r) => {
      if (r.net > 0) a.w += 1;
      else if (r.net === 0) a.t += 1;
      else a.l += 1;
      return a;
    },
    { w: 0, t: 0, l: 0 },
  );
  const hitRate = hist.length ? Math.round((stats.w / hist.length) * 100) : 0;

  async function guess(g: "small" | "big") {
    if (busy) return;
    setBusy(true);
    setErr(null);
    setFlash(null);
    setRes(null);
    setNum(null);
    idem.current =
      typeof crypto !== "undefined" && crypto.randomUUID
        ? crypto.randomUUID()
        : String(Date.now());
    try {
      const r = await api.post<GuessResult>("/api/v1/games/bigsmall", {
        bet,
        guess: g,
        idempotency_key: idem.current,
      });
      setNum(r.number);
      const settle = () => {
        setRes(r);
        setFlash({
          kind: r.tie ? "tie" : r.player_win ? "win" : "lose",
          text: r.tie
            ? fmtCur(tg.tie, { n: r.number }, currency)
            : r.player_win
              ? fmtCur(tg.win, { n: r.number, net: r.net }, currency)
              : fmtCur(tg.lose, { n: r.number, net: -r.net }, currency),
        });
        setStreak((s) => (r.player_win ? s + 1 : 0));
        setSessionPlays((n) => n + 1);
        setBusy(false);
        void loadMeta();
      };
      if (reduced) settle();
      else timer.current = window.setTimeout(settle, SPIN_MS);
    } catch (e) {
      // 1002 用后端具体原因（字典里的「参数校验失败」太笼统）
      setErr(
        e instanceof ApiError
          ? e.code === 1002
            ? e.message
            : (dict.errors[e.code] ?? e.message)
          : dict.common.networkError,
      );
      setBusy(false);
    }
  }

  return (
    <GameShell
      icon="🎯"
      title={tg.title}
      subtitle={`${tg.rule} · ${tg.winMult.replace("{n}", String(winMult))}`}
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
        <div className="flex w-full flex-col items-center gap-2">
          <GameStage
            art="/games/stage-bigsmall.jpg"
            className="h-[200px] w-full max-w-[460px]"
          >
            <RunwayOdometer number={num} spinning={busy} reduced={reduced} />
          </GameStage>
          <ResultFlash
            kind={flash?.kind ?? null}
            text={flash?.text ?? (busy ? tg.pending : null)}
          />
          {streak >= 3 && (
            <p className="text-xs font-bold text-[var(--warning)]">
              {tg.streak}
            </p>
          )}
          {err && <p className="text-xs text-danger">{err}</p>}
        </div>
      }
      controls={
        <div className={PANEL_LG_COL}>
          <ChipSelect
            value={bet}
            onChange={setBet}
            maxBet={maxBet}
            disabled={busy}
          />
          <div className="flex gap-2">
            <button
              type="button"
              onClick={() => guess("small")}
              disabled={busy}
              className="min-h-[56px] flex-1 rounded-full bg-sky text-base font-black text-white active:scale-[0.97] disabled:opacity-50"
            >
              {tg.small}
            </button>
            <button
              type="button"
              onClick={() => guess("big")}
              disabled={busy}
              className="min-h-[56px] flex-1 rounded-full bg-coral text-base font-black text-white active:scale-[0.97] disabled:opacity-50"
            >
              {tg.big}
            </button>
          </div>
        </div>
      }
      side={
        <div className={PANEL_LG}>
          <h2 className="mb-2 font-display text-base">{t.roadmap}</h2>
          <HistoryStrip rounds={hist} />
          {hist.length > 0 && (
            <p className="num mt-2 text-[11px] text-sub">
              {tg.statLine
                .replace("{n}", String(hist.length))
                .replace("{w}", String(stats.w))
                .replace("{t}", String(stats.t))
                .replace("{l}", String(stats.l))
                .replace("{p}", String(hitRate))}
            </p>
          )}
          <p className="mt-2 text-[11px] text-sub">{tg.rule}</p>
        </div>
      }
    />
  );
}
