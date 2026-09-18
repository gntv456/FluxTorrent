"use client";

import { useCallback, useEffect, useRef, useState } from "react";
import { api, ApiError } from "@/lib/api-client";
import { useI18n } from "@/i18n/client";
import { fmtCur } from "@/i18n/config";
import { GameShell, HistoryStrip, ResultFlash } from "@/components/game/game-kit";
import { JggGrid, type JggPrize } from "@/components/game/jgg-grid";

interface Overview {
  me?: { balance: number; today_net: number; today_plays: number; limit_left: number };
  jgg?: { ticket: number; prizes: JggPrize[] };
}

interface HistRow {
  ref_type: string | null;
  amount: number;
}

interface DrawResult {
  index: number;
  prize: string;
  payout: number;
  net: number;
}

/** 九宫格专注页：3×3 灯阵 + 跑马灯 + 翻牌揭晓 */
export default function JggPage() {
  const { dict, currency } = useI18n();
  const t = dict.games;
  const tj = dict.games.jgg;

  const [ov, setOv] = useState<Overview | null>(null);
  const [hist, setHist] = useState<HistRow[]>([]);
  const [busy, setBusy] = useState(false);
  const [res, setRes] = useState<DrawResult | null>(null);
  const [flash, setFlash] = useState<{ kind: "win" | "lose" | "tie" | "jackpot"; text: string } | null>(
    null,
  );
  const [err, setErr] = useState<string | null>(null);
  const [reduced, setReduced] = useState(false);
  const idem = useRef<string>("");

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
      setHist(await api.get<HistRow[]>("/api/v1/games/history?game=jgg&limit=10"));
    } catch {
      /* ignore */
    }
  }, []);

  useEffect(() => {
    void loadMeta();
  }, [loadMeta]);

  const prizes = ov?.jgg?.prizes ?? [];
  const ticket = ov?.jgg?.ticket ?? 100;
  const winRate = prizes.length
    ? (100 - (prizes[0]?.weight_permille ?? 0) / 10).toFixed(1)
    : "—";

  async function draw() {
    if (busy) return;
    setBusy(true);
    setErr(null);
    setFlash(null);
    setRes(null);
    idem.current =
      typeof crypto !== "undefined" && crypto.randomUUID ? crypto.randomUUID() : String(Date.now());
    try {
      const r = await api.post<DrawResult>("/api/v1/games/jgg", {
        idempotency_key: idem.current,
      });
      setRes(r);
    } catch (e) {
      setErr(e instanceof ApiError ? (dict.errors[e.code] ?? e.message) : dict.common.networkError);
      setBusy(false);
    }
  }

  function onLanded() {
    const r = res;
    setBusy(false);
    if (!r) return;
    const kind: "win" | "lose" | "tie" | "jackpot" =
      r.payout >= ticket * 10 ? "jackpot" : r.payout > ticket ? "win" : r.payout === ticket ? "tie" : "lose";
    setFlash({
      kind,
      text:
        r.payout >= ticket * 10
          ? fmtCur(tj.jackpot, { prize: r.prize, net: r.net }, currency)
          : r.payout > ticket
            ? fmtCur(tj.win, { prize: r.prize, net: r.net }, currency)
            : r.payout === ticket
              ? tj.again
              : tj.thanks,
    });
    void loadMeta();
  }

  return (
    <GameShell
      icon="🎰"
      title={tj.title}
      subtitle={`${tj.ticket} · ${tj.winRate.replace("{pct}", winRate)}`}
      balance={ov?.me?.balance ?? null}
      todayNet={ov?.me?.today_net ?? null}
      limitLeft={ov?.me?.limit_left ?? null}
      stage={
        <div className="flex w-full flex-col items-center gap-3">
          <JggGrid
            prizes={prizes}
            ticket={ticket}
            resultIndex={res?.index ?? null}
            busy={busy}
            disabled={prizes.length === 0}
            reduced={reduced}
            onDraw={draw}
            onLanded={onLanded}
            goLabel={tj.go}
            drawingLabel={tj.drawing}
          />
          <ResultFlash kind={flash?.kind ?? null} text={flash?.text ?? (busy ? tj.drawing : null)} />
          {err && <p className="text-xs text-danger">{err}</p>}
        </div>
      }
      controls={
        <div className="rounded-[var(--r-lg)] border border-line bg-[var(--surface-card)] p-4 shadow-[var(--shadow-card)]">
          <h2 className="mb-2 font-display text-base">{tj.prizePool}</h2>
          <div className="flex flex-wrap gap-1.5">
            {prizes.map((p, i) => (
              <span
                key={i}
                className={`rounded-full px-2 py-0.5 text-[11px] font-bold ${
                  p.payout >= 10
                    ? "bg-sun-soft text-[var(--warning)]"
                    : p.payout > 1
                      ? "bg-mint-soft text-[var(--mint)]"
                      : "bg-[var(--surface-sunken)] text-sub"
                }`}
              >
                {p.label} {(p.weight_permille / 10).toFixed(1)}%
              </span>
            ))}
          </div>
          <p className="mt-2 text-[11px] text-sub">{tj.poolNote}</p>
        </div>
      }
      side={
        <div className="rounded-[var(--r-lg)] border border-line bg-[var(--surface-card)] p-4 shadow-[var(--shadow-card)]">
          <h2 className="mb-2 font-display text-base">{t.history}</h2>
          <HistoryStrip items={hist} />
        </div>
      }
    />
  );
}
