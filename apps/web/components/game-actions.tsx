"use client";

import { useState } from "react";
import { api, ApiError } from "@/lib/api-client";
import { useI18n } from "@/i18n/client";
import { fmt } from "@/i18n/config";

function useBet(initial: number) {
  const { dict, currency } = useI18n();
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
      setResult(
        e instanceof ApiError
          ? (dict.errors[e.code] ?? e.message)
          : dict.common.networkError,
      );
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
  const { dict, currency } = useI18n();
  return (
    <div className="flex items-center gap-2">
      <span className="text-sm text-sub">{dict.games.bet}</span>
      <input
        type="number"
        min={1}
        max={1000}
        value={bet}
        onChange={(e) => setBet(Number(e.target.value) || 0)}
        aria-label={dict.games.betLabel.replace("{magic}", currency)}
        className="num min-h-[44px] w-24 rounded-[var(--r-sm)] border border-line px-3 text-right outline-none focus:ring-2 focus:ring-sky/40"
      />
      <span className="text-sm text-sub">{dict.common.spark.replace("{magic}", currency)}</span>
    </div>
  );
}

/** 刮刮乐（M24） */
export function ScratchCard() {
  const { dict, currency } = useI18n();
  const { bet, setBet, result, setResult, busy, play } = useBet(100);
  const [face, setFace] = useState<"🎁" | "💥" | "✨">("🎁");

  async function scratch() {
    const r = await play("/api/v1/games/scratch", { bet });
    if (!r) return;
    const net = r.net as number;
    setFace(net > (bet as number) ? "✨" : net > 0 ? "🎁" : "💥");
    setResult(
      net > 0
        ? fmt(dict.games.scratch.win, {
            mult: r.multiplier as number,
            net,
          })
        : net === 0
          ? dict.games.scratch.breakeven
          : fmt(dict.games.scratch.lose, { net: -net }),
    );
  }

  return (
    <section className="flex flex-col gap-3 rounded-[var(--r-lg)] border border-line bg-[var(--surface-card)] p-5 shadow-[var(--shadow-card)]">
      <h2 className="font-display text-xl">{dict.games.scratch.title}</h2>
      <div className="flex items-center justify-between">
        <span aria-hidden className="text-5xl">
          {face}
        </span>
        <BetInput bet={bet} setBet={setBet} />
      </div>
      <p className="text-xs text-sub">{dict.games.scratch.pool}</p>
      <button
        onClick={scratch}
        disabled={busy}
        className="min-h-[44px] rounded-full bg-coral font-bold text-white active:scale-[0.97] disabled:opacity-50"
      >
        {busy ? dict.games.scratch.busy : dict.games.scratch.action}
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
  const { dict, currency } = useI18n();
  const { bet, setBet, result, setResult, busy, play } = useBet(100);
  const [number, setNumber] = useState<number | null>(null);

  async function guess(g: "small" | "big") {
    const r = await play("/api/v1/games/bigsmall", { bet, guess: g });
    if (!r) return;
    setNumber(r.number as number);
    if (r.tie)
      setResult(fmt(dict.games.bigsmall.tie, { n: r.number as number }));
    else if (r.player_win)
      setResult(
        fmt(dict.games.bigsmall.win, {
          n: r.number as number,
          net: r.net as number,
        }),
      );
    else
      setResult(
        fmt(dict.games.bigsmall.lose, {
          n: r.number as number,
          net: -(r.net as number),
        }),
      );
  }

  return (
    <section className="flex flex-col gap-3 rounded-[var(--r-lg)] border border-line bg-[var(--surface-card)] p-5 shadow-[var(--shadow-card)]">
      <h2 className="font-display text-xl">{dict.games.bigsmall.title}</h2>
      <div className="flex items-center justify-between">
        <span aria-hidden className="num text-5xl font-black text-sky">
          {number ?? "?"}
        </span>
        <BetInput bet={bet} setBet={setBet} />
      </div>
      <p className="text-xs text-sub">{dict.games.bigsmall.rule}</p>
      <div className="grid grid-cols-2 gap-2">
        <button
          onClick={() => guess("small")}
          disabled={busy}
          className="min-h-[44px] rounded-full bg-sky font-bold text-white active:scale-[0.97] disabled:opacity-50"
        >
          {dict.games.bigsmall.small}
        </button>
        <button
          onClick={() => guess("big")}
          disabled={busy}
          className="min-h-[44px] rounded-full bg-sun font-bold text-ink active:scale-[0.97] disabled:opacity-50"
        >
          {dict.games.bigsmall.big}
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
  const { dict, currency } = useI18n();
  const [busy, setBusy] = useState(false);
  const [active, setActive] = useState<number | null>(null);
  const [won, setWon] = useState<number | null>(null);
  const [msg, setMsg] = useState<string | null>(null);

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
            ? fmt(dict.games.jgg.win, { prize: r.prize, net: r.net })
            : r.net === 0
              ? dict.games.jgg.again
              : dict.games.jgg.thanks,
        );
      }, 900);
    } catch (e) {
      setMsg(
        e instanceof ApiError
          ? (dict.errors[e.code] ?? e.message)
          : dict.common.networkError,
      );
    } finally {
      setTimeout(() => setBusy(false), 950);
    }
  }

  return (
    <section className="rounded-[var(--r-lg)] border border-line bg-[var(--surface-card)] p-4 shadow-[var(--shadow-card)]">
      <div className="flex items-center justify-between">
        <h2 className="font-display text-lg">{dict.games.jgg.title}</h2>
        <span className="num text-sm text-sky">{dict.games.jgg.ticket}</span>
      </div>
      <div className="mt-3 grid grid-cols-4 gap-2">
        {dict.games.jgg.cells.map((c, i) => (
          <div
            key={i}
            className={`flex min-h-[64px] items-center justify-center rounded-[var(--r-sm)] border text-center text-xs font-bold transition-all ${
              won === i
                ? "scale-105 border-sun bg-sun/20 text-ink shadow-[var(--shadow-hover)]"
                : active === i
                  ? "border-sky bg-sky-soft"
                  : "border-line bg-[var(--surface-card)] text-sub"
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
