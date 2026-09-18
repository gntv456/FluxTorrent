"use client";

import { useCallback, useEffect, useRef, useState } from "react";
import { api, ApiError } from "@/lib/api-client";
import { useI18n } from "@/i18n/client";
import { fmtCur } from "@/i18n/config";
import { ChipSelect, GameShell, HistoryStrip, ResultFlash } from "@/components/game/game-kit";
import { RunwayOdometer } from "@/components/game/runway";

interface Overview {
  max_bet: number;
  bigsmall?: { win_mult: number; expected_value: number };
  me?: { balance: number; today_net: number; today_plays: number; limit_left: number };
}

interface HistRow {
  ref_type: string | null;
  amount: number;
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
export default function BigSmallPage() {
  const { dict, currency } = useI18n();
  const t = dict.games;
  const tg = dict.games.bigsmall;

  const [ov, setOv] = useState<Overview | null>(null);
  const [hist, setHist] = useState<HistRow[]>([]);
  const [bet, setBet] = useState(100);
  const [busy, setBusy] = useState(false);
  const [num, setNum] = useState<number | null>(null);
  const [res, setRes] = useState<GuessResult | null>(null);
  const [flash, setFlash] = useState<{ kind: "win" | "lose" | "tie"; text: string } | null>(null);
  const [streak, setStreak] = useState(0);
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
      setHist(await api.get<HistRow[]>("/api/v1/games/history?game=bigsmall&limit=20"));
    } catch {
      /* ignore */
    }
  }, []);

  useEffect(() => {
    void loadMeta();
  }, [loadMeta]);

  useEffect(() => () => { if (timer.current) window.clearTimeout(timer.current); }, []);

  const maxBet = ov?.max_bet ?? 1000;
  // 赔率由后端下发（< 2.0，回收口径），不在前端写死
  const winMult = ov?.bigsmall?.win_mult ?? 1.9;

  async function guess(g: "small" | "big") {
    if (busy) return;
    setBusy(true);
    setErr(null);
    setFlash(null);
    setRes(null);
    setNum(null);
    idem.current =
      typeof crypto !== "undefined" && crypto.randomUUID ? crypto.randomUUID() : String(Date.now());
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
        setBusy(false);
        void loadMeta();
      };
      if (reduced) settle();
      else timer.current = window.setTimeout(settle, SPIN_MS);
    } catch (e) {
      setErr(e instanceof ApiError ? (dict.errors[e.code] ?? e.message) : dict.common.networkError);
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
      stage={
        <div className="flex w-full flex-col items-center gap-2">
          <RunwayOdometer number={num} spinning={busy} reduced={reduced} />
          <ResultFlash kind={flash?.kind ?? null} text={flash?.text ?? (busy ? tg.pending : null)} />
          {streak >= 3 && <p className="text-xs font-bold text-[var(--warning)]">{tg.streak}</p>}
          {err && <p className="text-xs text-danger">{err}</p>}
        </div>
      }
      controls={
        <div className="flex flex-col gap-3 rounded-[var(--r-lg)] border border-line bg-[var(--surface-card)] p-4 shadow-[var(--shadow-card)]">
          <ChipSelect value={bet} onChange={setBet} maxBet={maxBet} disabled={busy} />
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
        <div className="rounded-[var(--r-lg)] border border-line bg-[var(--surface-card)] p-4 shadow-[var(--shadow-card)]">
          <h2 className="mb-2 font-display text-base">{t.roadmap}</h2>
          <HistoryStrip items={hist} />
          <p className="mt-2 text-[11px] text-sub">{tg.rule}</p>
        </div>
      }
    />
  );
}
