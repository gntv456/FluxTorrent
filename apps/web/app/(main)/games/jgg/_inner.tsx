"use client";

import { PANEL_LG } from "@/lib/ui-classes";

import { useCallback, useEffect, useRef, useState } from "react";
import { api, ApiError } from "@/lib/api-client";
import { useTenDraw } from "@/lib/ten-draw";
import {
  SwCtaRow,
  SwHist,
  SwStatRow,
  SwTenModal,
  type SwHistRow,
} from "@/components/game/sw-panels";
import { useI18n } from "@/i18n/client";
import { fmtCur } from "@/i18n/config";
import { GameShell, PlayHint } from "@/components/game/game-kit";
import { ResultFlash } from "@/components/game/game-kit-feedback";
import { JggGrid, type JggPrize } from "@/components/game/jgg-grid";
import { SwPrizeStrip } from "@/components/game/sw-enrich";
import { GameStage } from "@/components/game/game-stage";

export interface Overview {
  me?: {
    balance: number;
    today_net: number;
    today_plays: number;
    limit_left: number;
  };
  jgg?: { ticket: number; prizes: JggPrize[] };
}

interface RoundRow {
  game: string;
  bet: number;
  payout: number;
  net: number;
  at?: string;
}

interface DrawResult {
  index: number;
  prize: string;
  payout: number;
  net: number;
  /** magic | item | fallback：物品中奖时 payout 为 0，只有 value 能区分「中了」与「没中」 */
  kind?: "magic" | "item" | "fallback";
  value?: number;
  fell_back?: string | null;
}

/** 九宫格专注页：3×3 灯阵 + 跑马灯 + 翻牌揭晓 */
export default function JggPage({
  initialOver,
}: {
  initialOver: Overview | null;
}) {
  const { dict, currency } = useI18n();
  const t = dict.games;
  const tj = dict.games.jgg;

  const [ov, setOv] = useState<Overview | null>(initialOver);
  const [hist, setHist] = useState<RoundRow[]>([]);
  const [busy, setBusy] = useState(false);
  const [res, setRes] = useState<DrawResult | null>(null);
  const [flash, setFlash] = useState<{
    kind: "win" | "lose" | "tie" | "jackpot";
    text: string;
  } | null>(null);
  const [err, setErr] = useState<string | null>(null);
  const [reduced, setReduced] = useState(false);
  /** 本次会话已完成的局数（防沉迷软提示用） */
  const [sessionPlays, setSessionPlays] = useState(0);
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
      setHist(
        await api.get<RoundRow[]>("/api/v1/games/rounds?game=jgg&limit=10"),
      );
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
  // 奖池条（样图④ 九宫格屏）：等值前 4 档
  const prizeStripItems = prizes
    .slice()
    .sort(
      (a, b) =>
        (b.value ?? b.payout * ticket) - (a.value ?? a.payout * ticket),
    )
    .slice(0, 4)
    .map((p) => ({
      ic: p.kind === "item" ? (p.icon ?? "🎁") : "💎",
      nm: p.label,
      odds: `${(p.weight_permille / 10).toFixed(1)}%`,
      tone:
        (p.value ?? p.payout * ticket) >= ticket * 10
          ? ("gold" as const)
          : (p.value ?? p.payout * ticket) >= ticket * 3
            ? ("lilac" as const)
            : ("sky" as const),
    }));

  async function draw() {
    if (busy) return;
    setBusy(true);
    setErr(null);
    setFlash(null);
    setRes(null);
    idem.current =
      typeof crypto !== "undefined" && crypto.randomUUID
        ? crypto.randomUUID()
        : String(Date.now());
    try {
      const r = await api.post<DrawResult>("/api/v1/games/jgg", {
        idempotency_key: idem.current,
      });
      setRes(r);
    } catch (e) {
      // 1002 用后端具体原因（如「票价超过单次上限」）
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

  function onLanded() {
    const r = res;
    setBusy(false);
    setSessionPlays((n) => n + 1);
    if (!r) return;
    // 必须按 value 判，不能按 payout：抽中物品时实付魔力是 0，
    // 按 payout 分档会把「中了免考核卡」渲染成「谢谢参与」。
    const won = r.value ?? r.payout;
    const kind: "win" | "lose" | "tie" | "jackpot" =
      won >= ticket * 10
        ? "jackpot"
        : won > ticket
          ? "win"
          : won === ticket
            ? "tie"
            : "lose";
    setFlash({
      kind,
      text:
        won >= ticket * 10
          ? fmtCur(tj.jackpot, { prize: r.prize, net: won - ticket }, currency)
          : won > ticket
            ? fmtCur(tj.win, { prize: r.prize, net: won - ticket }, currency)
            : won === ticket
              ? tj.again
              : tj.thanks,
    });
    void loadMeta();
  }

  // 十连：十次真实单抽（每次独立幂等键），结果汇总进浮层
  const ten = useTenDraw<DrawResult>({
    path: "/api/v1/games/jgg",
    onError: (m) => setErr(m),
    onDone: () => {
      setSessionPlays((n) => n + 10);
      void loadMeta();
    },
    netOf: (r) => (r.value ?? r.payout) - ticket,
    labelOf: (r) => r.prize,
  });

  const maxMult = prizes.length
    ? Math.max(
        ...prizes.map((p) => (p.value ?? (p.payout * ticket) / 1) / ticket),
      )
    : 0;
  const histRows: SwHistRow[] = hist.slice(0, 6).map((h, i) => ({
    t: (h.at ?? "").slice(11, 16) || "—",
    txt: h.net >= 0 ? tj.histWin : tj.histLose,
    net: h.net,
  }));

  return (
    <GameShell
      icon="🎰"
      title={tj.title}
      subtitle={`${tj.ticket} · ${tj.winRate.replace("{pct}", winRate)}`}
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
            art="/games/stage-jgg.jpg"
            className="w-full max-w-[460px] p-4"
          >
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
          </GameStage>
          <ResultFlash
            kind={flash?.kind ?? null}
            text={flash?.text ?? (busy ? tj.drawing : null)}
          />
          <SwStatRow
            items={[
              { lb: tj.statPlays, vl: String(ov?.me?.today_plays ?? 0) },
              {
                lb: tj.statMax,
                vl: maxMult > 0 ? `${maxMult.toFixed(1)}x` : "—",
                tone: "gold",
              },
              {
                lb: tj.statLeft,
                vl: ov?.me?.limit_left != null ? String(ov.me.limit_left) : "—",
                tone: "green",
              },
            ]}
          />
          <SwPrizeStrip label={tj.prizeLb} items={prizeStripItems} />
          {err && <p className="text-xs text-danger">{err}</p>}
        </div>
      }
      controls={
        <div className={PANEL_LG}>
          <h2 className="mb-2 font-display text-base">{tj.prizePool}</h2>
          <div className="flex flex-wrap gap-1.5">
            {prizes.map((p, i) => (
              <span
                key={i}
                className={`rounded-full px-2 py-0.5 text-[11px] font-bold ${
                  (p.value ?? p.payout * ticket) >= ticket * 10
                    ? "bg-sun-soft text-[var(--warning)]"
                    : (p.value ?? p.payout * ticket) > ticket
                      ? "bg-mint-soft text-[var(--mint)]"
                      : "bg-[var(--surface-sunken)] text-sub"
                }`}
              >
                {p.label} {(p.weight_permille / 10).toFixed(1)}%
              </span>
            ))}
          </div>
          <p className="mt-2 text-[11px] text-sub">{tj.poolNote}</p>
          <SwCtaRow
            primaryLabel={`${tj.go} · ${ticket}`}
            goldLabel={`${tj.tenBtn} · ${ticket * 10}`}
            onPrimary={draw}
            onGold={ten.run}
            primaryDisabled={busy || prizes.length === 0}
            goldDisabled={busy || ten.busy || prizes.length === 0}
            goldBusy={ten.busy}
          />
        </div>
      }
      side={
        <div className={PANEL_LG}>
          <h2 className="mb-2 font-display text-base">{t.history}</h2>
          <SwHist rows={histRows} empty={t.historyEmpty} />
        </div>
      }
      foot={
        <SwTenModal
          open={ten.open}
          title={tj.tenTitle}
          rows={ten.rows}
          totalLabel={tj.tenTotal}
          onClose={ten.close}
        />
      }
    />
  );
}
