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

/** 九宫格抽奖（M24 jgg 口径）：票价 100，8 格奖池 */
export function JggCard() {
  const [busy, setBusy] = useState(false);
  const [active, setActive] = useState<number | null>(null);
  const [won, setWon] = useState<number | null>(null);
  const [msg, setMsg] = useState<string | null>(null);

  const CELLS = [
    "🎁 100x", "💧 2x", "⭐ 50x", "🌱 3x",
    "🍀 10x", "❌ 谢谢", "🔁 再来", "🌻 5x",
  ];

  async function draw() {
    setBusy(true);
    setMsg(null);
    setWon(null);
    try {
      // 简易滚动动画：快速轮询点亮再定格
      let i = 0;
      const spin = setInterval(() => {
        setActive(i % 8);
        i += 1;
      }, 80);
      const r = await api.post<{ index: number; prize: string; net: number }>(
        "/api/v1/games/jgg",
      );
      setTimeout(() => {
        clearInterval(spin);
        setActive(r.index);
        setWon(r.index);
        setMsg(
          r.net > 0
            ? `「${r.prize}」 净赚 +${r.net} 火花！`
            : r.net === 0
              ? "「再来一次」 票价已返还"
              : "谢谢参与，下次一定～",
        );
      }, 900);
    } catch (e) {
      setMsg(e instanceof ApiError ? e.message : "网络异常");
    } finally {
      setTimeout(() => setBusy(false), 950);
    }
  }

  return (
    <section className="rounded-[var(--r-lg)] border border-line bg-white p-4 shadow-[var(--shadow-card)]">
      <div className="flex items-center justify-between">
        <h2 className="font-display text-lg">九宫格抽奖</h2>
        <span className="num text-sm text-sky">票价 100</span>
      </div>
      <div className="mt-3 grid grid-cols-4 gap-2">
        {CELLS.map((c, i) => (
          <div
            key={i}
            className={`flex min-h-[64px] items-center justify-center rounded-[var(--r-sm)] border text-center text-xs font-bold transition-all ${
              won === i
                ? "scale-105 border-sun bg-sun/20 text-ink shadow-[var(--shadow-hover)]"
                : active === i
                  ? "border-sky bg-sky-soft"
                  : "border-line bg-white text-sub"
            }`}
          >
            {c}
          </div>
        ))}
        <button
          onClick={draw}
          disabled={busy}
          className="flex min-h-[64px] items-center justify-center rounded-[var(--r-sm)] bg-coral text-sm font-bold text-white active:scale-[0.97] disabled:opacity-50"
        >
          {busy ? "…" : "GO!"}
        </button>
      </div>
      {msg && <p className="mt-2 text-sm text-sub">{msg}</p>}
    </section>
  );
}
