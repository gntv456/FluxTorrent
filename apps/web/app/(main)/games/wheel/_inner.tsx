"use client";

import { useCallback, useEffect, useState } from "react";
import { api, ApiError } from "@/lib/api-client";
import { GameTabs } from "@/components/game/game-tabs";
import { GameRecordsPanel } from "@/components/game/game-records";
import { ArcadeGlyph } from "@/components/arcade/arcade-glyph";
import { apiErrorMessage, useI18n } from "@/i18n/client";
import { GameShell, PlayHint } from "@/components/game/game-kit";
import { ResultFlash } from "@/components/game/game-kit-feedback";
import { StreakPill } from "@/components/game/streak-pill";
import { useRounds } from "@/components/game/use-rounds";
import { useWheelPlay } from "@/components/game/use-wheel-play";
import { WheelCtas } from "@/components/game/wheel-ctas";
import { WheelPoolPanel } from "@/components/game/wheel-pool";
import { prizeValue } from "@/lib/wheel-prize";
import type { WheelPrize } from "@/lib/wheel-prize";
import dynamic from "next/dynamic";
import { SkeletonBlock } from "@/components/ui/skeleton";

// W-33：转轮画布延迟加载（骨架防白屏）
const LuckyWheel = dynamic(() =>
  import("@/components/game/lucky-wheel").then((m) => m.LuckyWheel),
  { loading: () => <SkeletonBlock height={300} /> });

export interface Overview {
  me?: {
    balance: number; today_net: number;
    today_plays: number; limit_left: number;
  };
  wheel?: { ticket: number; prizes: WheelPrize[] };
}

type Tab = "play" | "pool" | "records";

/** 大转盘专注页：主玩法 / 奖池概率 / 记录 三标签（2026-11 样图定稿）。 */
export default function WheelFocusPage({
  initialOver,
}: {
  initialOver: Overview | null;
}) {
  const { dict, currency } = useI18n();
  const t = dict.games;
  const tw = dict.games.wheel;

  const [ov, setOv] = useState<Overview | null>(initialOver);
  const { hist, streak, reload: reloadRounds } = useRounds("wheel");
  const [tab, setTab] = useState<Tab>("play");
  const [batchBusy, setBatchBusy] = useState(false);
  const [reduced, setReduced] = useState(false);

  useEffect(() => {
    setReduced(window.matchMedia("(prefers-reduced-motion: reduce)").matches);
  }, []);

  const loadMeta = useCallback(async () => {
    try {
      setOv(await api.get<Overview>("/api/v1/games"));
    } catch {
      /* ignore */
    }
    void reloadRounds();
  }, [reloadRounds]);

  useEffect(() => {
    void loadMeta();
  }, [loadMeta]);

  const prizes = ov?.wheel?.prizes ?? [];
  const ticket = ov?.wheel?.ticket ?? 100;
  const poolReady = prizes.length > 0 && ticket > 0;
  const limitLeft = ov?.me?.limit_left ?? 0;
  const balance = ov?.me?.balance ?? 0;
  const todayNet = ov?.me?.today_net ?? 0;
  const totalW = prizes.reduce((s, p) => s + p.weight_permille, 0) || 1;
  // 物品档的 payout 恒为 0（价值在 anchor×件数里），不能按 payout 判「未中」；
  // 否则站长一配物品档，「综合中奖率」就会把它算成谢谢参与。
  const missW = prizes.reduce(
    (s, p) =>
      s + (p.kind !== "item" && p.payout === 0 ? p.weight_permille : 0),
    0,
  );
  const winRate = prizes.length
    ? (100 - (missW / totalW) * 100).toFixed(1)
    : "—";
  // 最高倍率按「实际折算价值 / 票价」算，物品档才不会在统计条里显示 0x。
  const maxMult = prizes.length && ticket > 0
    ? Math.max(...prizes.map((p) => prizeValue(p, ticket) / ticket))
    : 0;
  const netTone = todayNet < 0 ? "lose" : "green";

  const describeError = useCallback(
    (e: unknown) => (e instanceof ApiError
      ? e.code === 1002 ? e.message : apiErrorMessage(dict, e)
      : dict.common.networkError),
    [dict],
  );

  const play = useWheelPlay({
    ticket,
    labels: tw,
    currency,
    describeError,
    onSettled: loadMeta,
  });

  useEffect(() => {
    if (tab !== "play") play.stopAuto();
    // eslint-disable-next-line react-hooks/exhaustive-deps
  }, [tab]);

  const tabs = [
    { key: "play", label: t.sub.tabPlay, icon: "🎲" },
    { key: "pool", label: tw.tabPool, icon: "🎁" },
    { key: "records", label: t.sub.tabRecords, icon: "📜" },
  ];
  const tabBar = (
    <GameTabs tabs={tabs} active={tab} onChange={(k) => setTab(k as Tab)} />
  );

  const notice =
    ov && !poolReady
      ? tw.poolEmpty
      : limitLeft <= 0
        ? tw.noticeNoPlays
        : balance < ticket
          ? tw.noticePoor.replace("{n}", String(ticket))
          : null;

  const stage =
    tab === "pool" ? (
      <WheelPoolPanel
        prizes={prizes}
        ticket={ticket}
        labels={{
          picked: tw.poolPicked,
          gridTitle: tw.poolGrid,
          note: tw.poolNote,
          empty: tw.poolEmpty,
          metaItem: tw.metaItem,
          metaItemQty: tw.metaItemQty,
          metaMiss: tw.metaMiss,
          metaMult: tw.metaMult,
        }}
      />
    ) : tab === "records" ? (
      <GameRecordsPanel
        game="wheel"
        odds={{
          note: tw.recordsNote,
          label: t.oddsLink,
          onClick: () => setTab("pool"),
        }}
      />
    ) : (
      <div className="wp-stage">
        <LuckyWheel
          prizes={prizes}
          ticket={ticket}
          resultIndex={play.res?.index ?? null}
          spinKey={play.spinSeq}
          busy={play.busy}
          disabled={!poolReady || batchBusy}
          reduced={reduced}
          onDraw={() => void play.draw()}
          onLanded={play.landed}
          goLabel={tw.go}
          spinningLabel={tw.spinning}
          tapLabel={tw.tap}
        />
        <div className="wp-mid">
          <ResultFlash
            kind={play.flash?.kind ?? null}
            text={play.flash?.text ?? (play.busy ? tw.spinning : null)}
          />
          <StreakPill n={streak} label={t.streakLabel} />
        </div>
        <WheelCtas
          ticket={ticket}
          balance={balance}
          limitLeft={limitLeft}
          disabled={play.busy || play.auto || batchBusy || !poolReady}
          auto={play.auto}
          onGo={() => void play.draw()}
          onToggleAuto={play.toggleAuto}
          onDone={loadMeta}
          onBusyChange={setBatchBusy}
          describeError={describeError}
          labels={{
            go: tw.go,
            ten: tw.tenGo,
            hundred: tw.hundredGo,
            auto: tw.autoGo,
            autoStop: tw.autoStop,
            batchTitle: tw.batchTitle,
            batchTotal: tw.batchTotal,
            partial: tw.batchRest,
            capped: tw.batchCapped,
          }}
        />
        <div className="wp-stats">
          <div className="wp-stat">
            <div className="wp-stat-lb">{tw.statLeft}</div>
            <div className="wp-stat-vl num">{limitLeft}</div>
          </div>
          <div className="wp-stat">
            <div className="wp-stat-lb">{tw.statMax}</div>
            <div className="wp-stat-vl num gold">
              {maxMult.toFixed(1)}x
            </div>
          </div>
          <div className="wp-stat">
            <div className="wp-stat-lb">{tw.statNet}</div>
            <div className={`wp-stat-vl num ${netTone}`}>
              {todayNet > 0 ? "+" : ""}{todayNet}
            </div>
          </div>
        </div>
        {notice && <p className="wp-notice">{notice}</p>}
        {play.err && (
          <p role="alert" className="wp-error">{play.err}</p>
        )}
        {play.unknown && (
          <button
            type="button"
            onClick={() => void play.draw(true)}
            className="wp-retry"
          >
            {dict.common.retry}
          </button>
        )}
      </div>
    );

  return (
    <div className={`wheel-page tab-${tab}`}>
      <GameShell
        icon={<ArcadeGlyph k="wheel" />}
        title={tw.title}
        subtitle={tw.sub}
        headerLayout="center"
        badges={
          <>
            <span className="wp-badge">
              {tw.winRate.replace("{pct}", winRate)}
            </span>
            <span className="wp-badge ticket">
              {tw.ticketBadge.replace("{n}", String(ticket))}
            </span>
          </>
        }
        balance={ov?.me?.balance ?? null}
        todayNet={ov?.me?.today_net ?? null}
        limitLeft={ov?.me?.limit_left ?? null}
        balanceVariant="chips"
        balanceLabel={tw.balance}
        limitLabel={tw.statLeft}
        tabBar={tabBar}
        notice={tab === "play" ? (
          <PlayHint sessionPlays={play.sessionPlays} todayNet={todayNet} />
        ) : null}
        stage={stage}
        stageBox={tab === "play"}
        singleColumn
      />
    </div>
  );
}
