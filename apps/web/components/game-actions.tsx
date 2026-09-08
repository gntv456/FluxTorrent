"use client";

import { useState } from "react";
import { api, ApiError } from "@/lib/api-client";

function useBet(initial: number) {
  const [bet, setBet] = useState(initial);
  const [result, setResult] = useState<string | null>(null);
  const [busy, setBusy] = useState(false);

  async function play(path: string, body: Record<string, unknown>) {
    setBusy(true);
    setResult(null);
    try {
      const r = await api.post<{ net: number } & Record<string, unknown>>(
        path,
        body,
      );
      return r;
    } catch (e) {
      setResult(e instanceof ApiError ? e.message : "网络异常");
      return null;
    } finally {
      setBusy(false);
    }
  }

  return { bet, setBet, result, setResult, busy, play };
}

function BetInput({
  bet,
  setBet,
}: {
  bet: number;
  setBet: (n: number) => void;
}) {
  return (
    <div className="flex items-center gap-2">
      <span className="text-sm text-sub">下注</span>
      <input
        type="number"
        min={1}
        max={1000}
        value={bet}
        onChange={(e) => setBet(Number(e.target.value) || 0)}
        aria-label="下注火花数"
        className="num min-h-[44px] w-24 rounded-[var(--r-sm)] border border-line px-3 text-right outline-none focus:ring-2 focus:ring-sky/40"
      />
      <span className="text-sm text-sub">火花</span>
    </div>
  );
}

/** 刮刮乐（M24） */
export function ScratchCard() {
  const { bet, setBet, result, setResult, busy, play } = useBet(100);
  const [face, setFace] = useState<"🎁" | "💥" | "✨">("🎁");

  async function scratch() {
    const r = await play("/api/v1/games/scratch", { bet });
    if (!r) return;
    const net = r.net as number;
    setFace(net > (bet as number) ? "✨" : net > 0 ? "🎁" : "💥");
    setResult(
      net > 0
        ? `刮中 ${(r.multiplier as number)}x！净赢 ${net} 火花`
        : net === 0
          ? "保本，再来一次？"
          : `很遗憾，净输 ${-net} 火花`,
    );
  }

  return (
    <section className="flex flex-col gap-3 rounded-[var(--r-lg)] border border-line bg-white p-5 shadow-[var(--shadow-card)]">
      <h2 className="font-display text-xl">刮刮乐</h2>
      <div className="flex items-center justify-between">
        <span aria-hidden className="text-5xl">
          {face}
        </span>
        <BetInput bet={bet} setBet={setBet} />
      </div>
      <p className="text-xs text-sub">
        奖池：0.5x(30%) · 1x(15%) · 2x(8%) · 10x(2%)
      </p>
      <button
        onClick={scratch}
        disabled={busy}
        className="min-h-[44px] rounded-full bg-coral font-bold text-white active:scale-[0.97] disabled:opacity-50"
      >
        {busy ? "刮开中…" : "刮开"}
      </button>
      {result && (
        <p role="status" className="text-center text-sm font-bold text-ink">
          {result}
        </p>
      )}
    </section>
  );
}

/** 猜大小（M24） */
export function BigSmall() {
  const { bet, setBet, result, setResult, busy, play } = useBet(100);
  const [number, setNumber] = useState<number | null>(null);

  async function guess(g: "small" | "big") {
    const r = await play("/api/v1/games/bigsmall", { bet, guess: g });
    if (!r) return;
    setNumber(r.number as number);
    if (r.tie) setResult(`${r.number} 平局，返本`);
    else if (r.player_win) setResult(`${r.number} 猜中！净赢 ${r.net} 火花`);
    else setResult(`${r.number} 没猜中，净输 ${-(r.net as number)} 火花`);
  }

  return (
    <section className="flex flex-col gap-3 rounded-[var(--r-lg)] border border-line bg-white p-5 shadow-[var(--shadow-card)]">
      <h2 className="font-display text-xl">猜大小</h2>
      <div className="flex items-center justify-between">
        <span aria-hidden className="num text-5xl font-black text-sky">
          {number ?? "?"}
        </span>
        <BetInput bet={bet} setBet={setBet} />
      </div>
      <p className="text-xs text-sub">1-49 小 · 52-100 大 · 50/51 平局返本 · 猜中 2x</p>
      <div className="grid grid-cols-2 gap-2">
        <button
          onClick={() => guess("small")}
          disabled={busy}
          className="min-h-[44px] rounded-full bg-sky font-bold text-white active:scale-[0.97] disabled:opacity-50"
        >
          小
        </button>
        <button
          onClick={() => guess("big")}
          disabled={busy}
          className="min-h-[44px] rounded-full bg-sun font-bold text-ink active:scale-[0.97] disabled:opacity-50"
        >
          大
        </button>
      </div>
      {result && (
        <p role="status" className="text-center text-sm font-bold text-ink">
          {result}
        </p>
      )}
    </section>
  );
}
