"use client";

import { PANEL_LG, PANEL_LG_COL } from "@/lib/ui-classes";

import { useCallback, useEffect, useRef, useState } from "react";
import { api, ApiError } from "@/lib/api-client";
import { useI18n } from "@/i18n/client";
import { fmtCur } from "@/i18n/config";
import { GameShell, PlayHint } from "@/components/game/game-kit";
import { GameTabs } from "@/components/game/game-tabs";
import { GameRecordsPanel } from "@/components/game/game-records";
import { ArcadeGlyph } from "@/components/arcade/arcade-glyph";
import { ResultFlash } from "@/components/game/game-kit-feedback";
import { ScratchCoat, type CoatOutcome } from "@/components/game/scratch-coat";
import {
  SwCtaRow,
  SwHist,
  SwStatRow,
  SwTenModal,
  type SwHistRow,
} from "@/components/game/sw-panels";
import { useTenDraw } from "@/lib/ten-draw";
import { scratchPoolText, type ScratchPrize } from "@/lib/games";
import { SwPrizeStrip } from "@/components/game/sw-enrich";

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
  at?: string;
}

interface ScratchResp {
  multiplier: number;
  payout: number;
  net: number;
  prize: string;
}

/** 星尘信封（样图②）：票种三档 → 3×2 涂层格揭开 → 命中高亮。
 *  一局仍是一次真实单抽（服务端买卡时定死结果），六格只是揭开仪式。 */
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
  const [outcome, setOutcome] = useState<CoatOutcome | null>(null);
  const [hitIndex, setHitIndex] = useState<number | null>(null);
  const [revealed, setRevealed] = useState<number[]>([]);
  const [round, setRound] = useState(0);
  const [flash, setFlash] = useState<{
    kind: "win" | "lose" | "tie" | "jackpot";
    text: string;
  } | null>(null);
  const [err, setErr] = useState<string | null>(null);
  // 样图手机版：主玩法 / 记录图鉴 双 tab
  const [tab, setTab] = useState("play");
  const [sessionPlays, setSessionPlays] = useState(0);
  const idem = useRef<string>("");

  const loadMeta = useCallback(async () => {
    try {
      setOv(await api.get<Overview>("/api/v1/games"));
    } catch {
      /* 顶栏数据取不到不影响玩 */
    }
    try {
      setHist(
        await api.get<RoundRow[]>("/api/v1/games/rounds?game=scratch&limit=10"),
      );
    } catch {
      /* ignore */
    }
  }, []);

  useEffect(() => {
    void loadMeta();
  }, [loadMeta]);

  const prizes = ov?.scratch?.prizes ?? [];
  // payout 即票价倍数（0.5x/2x/...），物品位 payout=0 不进最高倍率
  const maxMult = prizes.length ? Math.max(...prizes.map((p) => p.payout)) : 0;
  // 奖池条（样图② prize-strip）：按等值取前 4 档，千分权重换算成 %
  const prizeStripItems = prizes
    .slice()
    .sort((a, b) => (b.value ?? b.payout) - (a.value ?? a.payout))
    .slice(0, 4)
    .map((p) => ({
      ic: p.kind === "item" ? "🎁" : "💎",
      nm: p.label,
      odds: `${(p.weight_permille / 10 || 0).toFixed(1)}%`,
      tone:
        (p.rarity ?? 0) >= 4
          ? ("gold" as const)
          : (p.rarity ?? 0) >= 3
            ? ("lilac" as const)
            : ("sky" as const),
    }));
  const histRows: SwHistRow[] = hist.slice(0, 6).map((h) => ({
    t: (h.at ?? "").slice(11, 16) || "—",
    txt: h.net > 0 ? ts.histWin : ts.histLose,
    net: h.net,
  }));

  async function buy() {
    setErr(null);
    setFlash(null);
    setPhase("buying");
    idem.current =
      typeof crypto !== "undefined" && crypto.randomUUID
        ? crypto.randomUUID()
        : String(Date.now());
    try {
      const r = await api.post<ScratchResp>("/api/v1/games/scratch", {
        bet,
        idempotency_key: idem.current,
      });
      const meta = prizes.find((p) => p.label === r.prize);
      setOutcome({
        mult: r.multiplier,
        icon: meta?.icon,
        rarity: meta?.rarity ?? 1,
        net: r.net,
      });
      setHitIndex(Math.floor(Math.random() * 6));
      setRevealed([]);
      setRound((n) => n + 1);
      setPhase("scratchable");
    } catch (e) {
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

  const ten = useTenDraw<ScratchResp>({
    path: "/api/v1/games/scratch",
    body: { bet },
    onError: (m) => setErr(m),
    onDone: () => {
      setSessionPlays((n) => n + 10);
      void loadMeta();
    },
    netOf: (r) => r.net,
    labelOf: (r) => `${r.prize} ${r.multiplier.toFixed(1)}x`,
  });

  function finishTicket() {
    const o = outcome;
    setPhase("done");
    setSessionPlays((n) => n + 1);
    if (!o) return;
    const kind: "win" | "lose" | "tie" | "jackpot" =
      o.mult >= 2
        ? "jackpot"
        : o.net > 0
          ? "win"
          : o.net === 0
            ? "tie"
            : "lose";
    setFlash({
      kind,
      text:
        o.mult >= 2
          ? fmtCur(ts.jackpot, { mult: o.mult, net: o.net }, currency)
          : o.net > 0
            ? fmtCur(ts.win, { mult: o.mult, net: o.net }, currency)
            : o.net === 0
              ? ts.breakeven
              : fmtCur(ts.lose, { net: -o.net }, currency),
    });
    void loadMeta();
  }

  function onReveal(i: number) {
    // updater 必须纯净（StrictMode 会双调用）：只算下一个集合，
    // 「第 6 格 → 结算」的副作用在 setState 外面判（2026-10 审计 P2）
    const next = revealed.includes(i) ? revealed : [...revealed, i];
    setRevealed(next);
    if (next.length >= 6) finishTicket();
  }

  if (tab === "records") {
    return (
      <div className="flex flex-col gap-4">
        <GameTabs
          tabs={[
            { key: "play", label: dict.games.sub.tabPlay, icon: "🎲" },
            { key: "records", label: dict.games.sub.tabRecords, icon: "📜" },
          ]}
          active={tab}
          onChange={setTab}
        />
        <GameRecordsPanel game="scratch" />
      </div>
    );
  }
  return (
    <GameShell
      icon={<ArcadeGlyph k="scratch" />}
      title={ts.title}
      subtitle={scratchPoolText(ov?.scratch?.prizes, ts.poolLabel) || ts.pool}
      balance={ov?.me?.balance ?? null}
      todayNet={ov?.me?.today_net ?? null}
      limitLeft={ov?.me?.limit_left ?? null}
      notice={
        <>
          <GameTabs
            tabs={[
              { key: "play", label: dict.games.sub.tabPlay, icon: "🎲" },
              { key: "records", label: dict.games.sub.tabRecords, icon: "📜" },
            ]}
            active={tab}
            onChange={setTab}
          />
          <PlayHint
            sessionPlays={sessionPlays}
            todayNet={ov?.me?.today_net ?? null}
          />
        </>
      }
      stage={
        <div className="flex w-full flex-col items-center gap-3">
          <div className="sw-envelope">
            <div className="sw-env-head">
              <span>
                {ts.headTicket
                  .replace("{n}", String(round + 1))
                  .replace("{bet}", String(bet))}
              </span>
              <span className="num">
                {ts.tierMax.replace("{n}", maxMult.toFixed(0))}
              </span>
            </div>
            <ScratchCoat
              outcome={phase === "idle" ? null : outcome}
              hitIndex={hitIndex}
              revealed={revealed}
              onReveal={onReveal}
              decoyLabel={ts.coatDecoy}
              disabled={phase !== "scratchable"}
            />
            <div className="sw-env-info">
              <span>
                {ts.coatProgress
                  .replace("{a}", String(revealed.length))
                  .replace("{b}", "6")}
              </span>
              {phase === "scratchable" && (
                <button
                  type="button"
                  className="sw-reveal-all"
                  onClick={() => {
                    for (let i = 0; i < 6; i++) onReveal(i);
                  }}
                >
                  {ts.autoScratch}
                </button>
              )}
            </div>
          </div>
          <ResultFlash kind={flash?.kind ?? null} text={flash?.text ?? null} />
          {err && <p className="text-xs text-danger">{err}</p>}
        </div>
      }
      controls={
        <div className={PANEL_LG_COL}>
          {/* 票种三档（样图②）：10 / 100 / 1000 */}
          <div className="sw-tiers">
            {[10, 100, 1000].map((v) => (
              <button
                key={v}
                type="button"
                onClick={() => setBet(v)}
                disabled={phase === "buying" || phase === "scratchable"}
                aria-pressed={bet === v}
                className={`sw-tier${
                  bet === v ? (v === 1000 ? " on-gold" : " on-mint") : ""
                }`}
              >
                <div className="sw-tier-b num">
                  {v} {currency}
                </div>
                <div className="sw-tier-s">
                  {ts.tierMax.replace("{n}", maxMult.toFixed(0))}
                </div>
              </button>
            ))}
          </div>
          <SwStatRow
            items={[
              { lb: ts.statPlays, vl: String(ov?.me?.today_plays ?? 0) },
              {
                lb: ts.statMax,
                vl: maxMult > 0 ? `${maxMult.toFixed(0)}x` : "—",
                tone: "gold",
              },
              {
                lb: ts.statLeft,
                vl: ov?.me?.limit_left != null ? String(ov.me.limit_left) : "—",
                tone: "green",
              },
            ]}
          />
          <SwPrizeStrip label={ts.prizeLb} items={prizeStripItems} />
          <SwCtaRow
            primaryLabel={`${ts.buy.replace("{n}", String(bet))}`}
            goldLabel={`${ts.tenBtn} · ${bet * 10}`}
            onPrimary={buy}
            onGold={ten.run}
            primaryDisabled={phase === "buying" || phase === "scratchable"}
            goldDisabled={
              ten.busy || phase === "buying" || phase === "scratchable"
            }
            goldBusy={ten.busy}
          />
        </div>
      }
      side={
        <div className={PANEL_LG}>
          <h2 className="mb-2 font-display text-base">{t.history}</h2>
          <SwHist rows={histRows} empty={t.historyEmpty} />
          <p className="mt-3 text-[11px] text-sub">
            {scratchPoolText(ov?.scratch?.prizes, ts.poolLabel) || ts.pool}
          </p>
        </div>
      }
      foot={
        <SwTenModal
          open={ten.open}
          title={ts.tenTitle}
          rows={ten.rows}
          totalLabel={ts.tenTotal.replace("{magic}", currency)}
          onClose={ten.close}
        />
      }
    />
  );
}
