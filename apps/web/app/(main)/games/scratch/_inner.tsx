"use client";
import {
  BTN_RETRY_PILL,
  PANEL_WARN_ORANGE,
} from "@/lib/ui-classes";

import { useCallback, useEffect, useRef, useState } from "react";
import { api, ApiError, isRetryableNetError } from "@/lib/api-client";
import { apiErrorMessage, useI18n } from "@/i18n/client";
import { fmtCur } from "@/i18n/config";
import { GameShell, PlayHint } from "@/components/game/game-kit";
import { GameTabs } from "@/components/game/game-tabs";
import { GameRecordsPanel } from "@/components/game/game-records";
import { ArcadeGlyph } from "@/components/arcade/arcade-glyph";
import type { CoatOutcome } from "@/components/game/scratch-coat";
import { SwTenModal } from "@/components/game/sw-panels";
import { useTenDraw } from "@/lib/ten-draw";
import { scratchPoolText, type ScratchPrize } from "@/lib/games";
import { ScratchPlayCard } from "@/components/game/scratch-play-card";
import { ScratchPoolPanel } from "@/components/game/scratch-pool-panel";
import {
  ScratchWinModal,
  type ScratchWinKind,
} from "@/components/game/scratch-win-modal";
import { GameRules } from "@/components/game/game-rules";
import { useRounds } from "@/components/game/use-rounds";
import { clientUuid } from "@/lib/client-uuid";

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
interface ScratchResp {
  multiplier: number;
  payout: number;
  net: number;
  prize: string;
}

type Tab = "play" | "pool" | "records";

/** 星尘信封：票种三档 → 3×2 涂层格揭开 → 命中高亮。
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
  const { reload: reloadRounds } = useRounds("scratch");
  const [bet, setBet] = useState(100);
  const [phase, setPhase] = useState<
    "idle" | "buying" | "scratchable" | "done"
  >("idle");
  const [outcome, setOutcome] = useState<CoatOutcome | null>(null);
  const outcomeRef = useRef<CoatOutcome | null>(null);
  const [hitIndex, setHitIndex] = useState<number | null>(null);
  const [revealed, setRevealed] = useState<number[]>([]);
  const [round, setRound] = useState(0);
  const [flash, setFlash] = useState<{
    kind: "win" | "lose" | "tie" | "jackpot";
    text: string;
  } | null>(null);
  const [err, setErr] = useState<string | null>(null);
  const [tab, setTab] = useState<Tab>("play");
  const [sessionPlays, setSessionPlays] = useState(0);
  const idem = useRef<string>("");
  const [unknown, setUnknown] = useState(false);
  const [auto, setAuto] = useState(false);
  const autoRef = useRef(false);
  const autoTimer = useRef<number | null>(null);
  const buyRef = useRef<() => void>(() => {});
  const [winModal, setWinModal] = useState<{
    open: boolean;
    kind: ScratchWinKind;
    amount: string;
  }>({ open: false, kind: "lose", amount: "0" });

  const loadMeta = useCallback(async () => {
    try {
      setOv(await api.get<Overview>("/api/v1/games"));
    } catch {
      /* 顶栏数据取不到不影响玩 */
    }
    void reloadRounds();
  }, [reloadRounds]);

  const stopAuto = useCallback(() => {
    autoRef.current = false;
    setAuto(false);
    if (autoTimer.current !== null) {
      window.clearTimeout(autoTimer.current);
      autoTimer.current = null;
    }
  }, []);

  useEffect(
    () => () => {
      if (autoTimer.current !== null) window.clearTimeout(autoTimer.current);
    },
    [],
  );

  useEffect(() => {
    void loadMeta();
  }, [loadMeta]);

  const prizes = ov?.scratch?.prizes ?? [];
  const maxBet = ov?.max_bet ?? 1000;
  const betAllowed = bet <= maxBet;
  const maxMult = prizes.length
    ? Math.max(...prizes.map((p) => p.payout)) : 0;
  const totalW = prizes.reduce((s, p) => s + p.weight_permille, 0) || 1;
  const missW = prizes.reduce(
    (s, p) => s + (p.kind !== "item" && p.payout === 0 ? p.weight_permille : 0),
    0,
  );
  const winRate = prizes.length
    ? (100 - (missW / totalW) * 100).toFixed(1)
    : "—";

  async function buy(reuse = false) {
    setErr(null);
    setFlash(null);
    setWinModal((m) => ({ ...m, open: false }));
    setPhase("buying");
    setUnknown(false);
    if (!reuse || !idem.current) idem.current = clientUuid();
    try {
      const r = await api.post<ScratchResp>("/api/v1/games/scratch", {
        bet,
        idempotency_key: idem.current,
      });
      const meta = prizes.find((p) => p.label === r.prize);
      const next: CoatOutcome = {
        mult: r.multiplier,
        icon: meta?.icon,
        rarity: meta?.rarity ?? 1,
        net: r.net,
      };
      setOutcome(next);
      outcomeRef.current = next;
      setHitIndex(Math.floor(Math.random() * 6));
      setRevealed([]);
      setRound((n) => n + 1);
      setPhase("scratchable");
      idem.current = "";
      void loadMeta();
      if (autoRef.current) {
        autoTimer.current = window.setTimeout(() => revealAll(), 500);
      }
    } catch (e) {
      setErr(
        e instanceof ApiError
          ? e.code === 1002
            ? e.message
            : apiErrorMessage(dict, e)
          : dict.common.networkError,
      );
      if (isRetryableNetError(e)) setUnknown(true);
      stopAuto();
      setPhase("idle");
    }
  }
  buyRef.current = () => {
    void buy();
  };

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
    const o = outcomeRef.current;
    setPhase("done");
    setSessionPlays((n) => n + 1);
    if (!o) return;
    const kind: ScratchWinKind =
      o.mult >= 2
        ? "jackpot"
        : o.net > 0
          ? "win"
          : o.net === 0
            ? "tie"
            : "lose";
    const text =
      o.mult >= 2
        ? fmtCur(ts.jackpot, { mult: o.mult, net: o.net }, currency)
        : o.net > 0
          ? fmtCur(ts.win, { mult: o.mult, net: o.net }, currency)
          : o.net === 0
            ? ts.breakeven
            : fmtCur(ts.lose, { net: -o.net }, currency);
    setFlash({ kind, text });
    // 手动单张结算弹独立浮层；自动连刮进行中不打断循环。
    if (!autoRef.current) {
      setWinModal({
        open: true,
        kind,
        amount: o.net > 0 ? `+${o.net}` : String(o.net),
      });
    }
    void loadMeta();
    if (autoRef.current) {
      autoTimer.current = window.setTimeout(() => buyRef.current(), 1300);
    }
  }

  function revealAll() {
    setRevealed([0, 1, 2, 3, 4, 5]);
    finishTicket();
  }

  function onReveal(i: number) {
    const next = revealed.includes(i) ? revealed : [...revealed, i];
    setRevealed(next);
    if (next.length >= 6) finishTicket();
  }

  const toggleAuto = useCallback(() => {
    const next = !autoRef.current;
    autoRef.current = next;
    setAuto(next);
    if (next && (phase === "idle" || phase === "done")) buyRef.current();
  }, [phase]);

  const tabs = [
    { key: "play", label: t.sub.tabPlay, icon: "🎲" },
    { key: "pool", label: ts.tabPool, icon: "🎁" },
    { key: "records", label: t.sub.tabRecords, icon: "📜" },
  ];

  const closeModal = useCallback(() => {
    setWinModal((m) => ({ ...m, open: false }));
  }, []);

  const stage =
    tab === "pool" ? (
      <>
        <ScratchPoolPanel
          prizes={prizes}
          labels={{
            gridTitle: ts.prizeLb,
            note: ts.poolNote,
            empty: t.historyEmpty,
            metaMiss: ts.coatDecoy,
          }}
        />
        <GameRules game="scratch" />
      </>
    ) : tab === "records" ? (
      <GameRecordsPanel
        game="scratch"
        odds={{
          note: ts.poolNote,
          label: t.oddsLink,
          onClick: () => setTab("pool"),
        }}
      />
    ) : (
      <ScratchPlayCard
        bet={bet}
        onBet={setBet}
        maxBet={maxBet}
        phase={phase}
        outcome={outcome}
        hitIndex={hitIndex}
        revealed={revealed}
        onReveal={onReveal}
        onRevealAll={revealAll}
        headTicket={ts.headTicket
          .replace("{n}", String(round + 1))
          .replace("{bet}", String(bet))}
        tierMaxText={ts.tierMax.replace("{n}", maxMult.toFixed(0))}
        coatProgress={ts.coatProgress
          .replace("{a}", String(revealed.length))
          .replace("{b}", "6")}
        decoyLabel={ts.coatDecoy}
        autoScratchLabel={ts.autoScratch}
        stats={{
          playsLb: ts.statPlays,
          playsVal: String(ov?.me?.today_plays ?? 0),
          maxLb: ts.statMax,
          maxVal: maxMult > 0 ? `${maxMult.toFixed(0)}x` : "—",
          leftLb: ts.statLeft,
          leftVal: ov?.me?.limit_left != null ? String(ov.me.limit_left) : "—",
        }}
        cta={{
          primaryLabel: ts.buy.replace("{n}", String(bet)),
          goldLabel: `${ts.tenBtn} · ${bet * 10}`,
          onPrimary: () => void buy(),
          onGold: ten.run,
          primaryDisabled:
            phase === "buying" ||
            phase === "scratchable" ||
            !betAllowed ||
            auto,
          goldDisabled:
            ten.busy ||
            phase === "buying" ||
            phase === "scratchable" ||
            !betAllowed ||
            auto,
          goldBusy: ten.busy,
          autoLabel: auto ? ts.autoStop : ts.autoGo,
          autoActive: auto,
          autoDisabled:
            auto
              ? false
              : phase === "buying" || phase === "scratchable" || !betAllowed,
          onAuto: toggleAuto,
        }}
        flash={flash}
        bannerTitle={ts.title}
        tagline={ts.bannerTagline}
        currency={currency}
      />
    );

  const modalTitle =
    winModal.kind === "jackpot"
      ? ts.mJackpot
      : winModal.kind === "win"
        ? ts.mWin
        : winModal.kind === "tie"
          ? ts.mTie
          : ts.mLose;

  return (
    <div className={`scratch-promo tab-${tab}`}>
      <GameShell
        icon={<ArcadeGlyph k="scratch" />}
        title={ts.title}
        subtitle={
          scratchPoolText(ov?.scratch?.prizes, ts.poolLabel) || ts.pool
        }
        headerLayout="center"
        badges={
          <span className="sp-badge">
            {ts.winRate.replace("{pct}", winRate)}
          </span>
        }
        balance={ov?.me?.balance ?? null}
        todayNet={ov?.me?.today_net ?? null}
        limitLeft={ov?.me?.limit_left ?? null}
        balanceVariant="chips"
        tabBar={
          <GameTabs
            tabs={tabs}
            active={tab}
            onChange={(k) => {
              setTab(k as Tab);
              if (k !== "play") stopAuto();
            }}
          />
        }
        notice={
          tab === "play" ? (
            <PlayHint
              sessionPlays={sessionPlays}
              todayNet={ov?.me?.today_net ?? null}
            />
          ) : null
        }
        stage={stage}
        stageBox={tab === "play"}
        singleColumn
      />
      {tab === "play" && err && (
        <p className="sp-error" role="alert">
          {err}
        </p>
      )}
      {tab === "play" && unknown && (
        <div className={PANEL_WARN_ORANGE}>
          <p>{dict.review.resultUnknownBody}</p>
          <button
            type="button"
            className={BTN_RETRY_PILL}
            onClick={() => void buy(true)}
          >
            {dict.review.retrySameKey}
          </button>
        </div>
      )}
      <SwTenModal
        open={ten.open}
        title={ts.tenTitle}
        rows={ten.rows}
        totalLabel={ts.tenTotal.replace("{magic}", currency)}
        onClose={ten.close}
      />
      <ScratchWinModal
        open={winModal.open}
        kind={winModal.kind}
        title={modalTitle}
        desc={flash?.text ?? ""}
        amount={winModal.amount}
        amountSuffix={currency}
        againLabel={ts.mAgain}
        tenLabel={ts.tenBtn}
        closeLabel={ts.winClose}
        onAgain={() => {
          closeModal();
          void buy();
        }}
        onTen={() => {
          closeModal();
          ten.run();
        }}
        onClose={closeModal}
      />
    </div>
  );
}
