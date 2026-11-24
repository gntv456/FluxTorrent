"use client";

import { useCallback, useEffect, useState } from "react";
import Link from "next/link";
import { api, ApiError } from "@/lib/api-client";
import { GameTabs } from "@/components/game/game-tabs";
import { GameRecordsPanel } from "@/components/game/game-records";
import { ArcadeGlyph } from "@/components/arcade/arcade-glyph";
import { apiErrorMessage, useI18n } from "@/i18n/client";
import { GameShell, PlayHint } from "@/components/game/game-kit";
import { StreakPill } from "@/components/game/streak-pill";
import { useRounds } from "@/components/game/use-rounds";
import { useMolePlay } from "@/components/game/use-mole-play";
import { MoleBoard } from "@/components/game/mole-board";
import { MolePoolPanel } from "@/components/game/mole-pool";
import type { MolePrize } from "@/lib/mole-tier";

export interface Overview {
  me?: {
    balance: number; today_net: number;
    today_plays: number; limit_left: number;
  };
  mole?: {
    ticket: number;
    prizes: MolePrize[];
    spawns: number;
    bomb_spawns: number;
    round_ms: number;
    bomb_penalty: number;
  };
}

type Tab = "play" | "pool" | "records";

/** 打地鼠专注页：主玩法 / 奖池概率 / 记录 三标签。 */
export default function MoleFocusPage({
  initialOver,
}: {
  initialOver: Overview | null;
}) {
  const { dict, currency } = useI18n();
  const t = dict.games;
  const tm = dict.games.mole;

  const [ov, setOv] = useState<Overview | null>(initialOver);
  const { streak, reload: reloadRounds } = useRounds("mole");
  const [tab, setTab] = useState<Tab>("play");
  const [reduced, setReduced] = useState(false);

  useEffect(() => {
    setReduced(window.matchMedia("(prefers-reduced-motion: reduce)").matches);
  }, []);

  const loadMeta = useCallback(async () => {
    try {
      setOv(await api.get<Overview>("/api/v1/games"));
    } catch {
      /* 刷新失败保留上一次读数，不把页面打空 */
    }
    void reloadRounds();
  }, [reloadRounds]);

  useEffect(() => {
    void loadMeta();
  }, [loadMeta]);

  const prizes = ov?.mole?.prizes ?? [];
  const ticket = ov?.mole?.ticket ?? 100;
  const spawns = ov?.mole?.spawns ?? 12;
  const bombSpawns = ov?.mole?.bomb_spawns ?? 2;
  const roundMs = ov?.mole?.round_ms ?? 30000;
  const bombPenalty = ov?.mole?.bomb_penalty ?? 25;
  const limitLeft = ov?.me?.limit_left ?? 0;
  const balance = ov?.me?.balance ?? 0;
  const todayNet = ov?.me?.today_net ?? 0;
  const maxMult = prizes.length
    ? Math.max(...prizes.map((p) => p.payout)) : 0;

  const describeError = useCallback(
    (e: unknown) => (e instanceof ApiError
      ? e.code === 1002 ? e.message : apiErrorMessage(dict, e)
      : dict.common.networkError),
    [dict],
  );

  const play = useMolePlay({ describeError, onSettled: loadMeta });

  useEffect(() => {
    if (tab !== "play") play.stop();
    // eslint-disable-next-line react-hooks/exhaustive-deps
  }, [tab]);

  const tabs = [
    { key: "play", label: t.sub.tabPlay, icon: "🔨" },
    { key: "pool", label: tm.tabPool, icon: "🎁" },
    { key: "records", label: t.sub.tabRecords, icon: "📜" },
  ];
  const tabBar = (
    <GameTabs tabs={tabs} active={tab} onChange={(k) => setTab(k as Tab)} />
  );

  const notice =
    limitLeft <= 0
      ? tm.noticeNoPlays
      : balance < ticket
        ? tm.noticePoor.replace("{n}", String(ticket))
        : null;

  const playing = play.phase === "playing" || play.phase === "settling";
  const started = play.hits > 0 || play.score !== 0;
  const stage =
    tab === "pool" ? (
      <MolePoolPanel
        prizes={prizes}
        ticket={ticket}
        spawns={spawns}
        bombSpawns={bombSpawns}
        bombPenalty={bombPenalty}
        labels={{
          picked: tm.poolPicked,
          gridTitle: tm.poolGrid,
          note: tm.poolNote,
          empty: tm.poolEmpty,
          metaMiss: tm.metaMiss,
          metaMult: tm.metaMult,
          metaPenalty: tm.metaPenalty,
          bombName: tm.bombName,
          bombNote: tm.bombNote,
          ceiling: tm.ceiling,
        }}
      />
    ) : tab === "records" ? (
      <GameRecordsPanel
        game="mole"
        odds={{
          note: tm.recordsNote,
          label: t.oddsLink,
          onClick: () => setTab("pool"),
        }}
      />
    ) : (
      <div className="mo-stage">
        <div className="mo-stage-wrap">
          <MoleBoard
            active={play.active}
            idle={play.phase === "idle"}
            ticket={ticket}
            left={play.phase === "idle" ? roundMs : play.left}
            roundMs={roundMs}
            score={play.score}
            combo={play.combo}
            miss={play.miss}
            playing={playing || play.res !== null}
            reduced={reduced}
            labels={{
              hole: tm.holeLabel,
              bomb: tm.bombName,
              left: tm.statLeftLabel,
              score: tm.statScore,
              combo: tm.combo,
              idle: tm.idleHint,
            }}
            onWhack={play.whack}
          />
          {play.res && play.phase === "settled" && (
            <div className="mo-result" role="status" aria-live="polite">
              <div className="mo-res-top">{tm.resultTitle}</div>
              <div className="mo-res-score num">
                {play.res.score}
                <span className="mo-res-unit">{tm.scoreUnit}</span>
              </div>
              <div className={`mo-res-net num ${
                play.res.net > 0 ? "win" : play.res.net < 0 ? "lose" : ""
              }`}>
                {play.res.net > 0 ? "+" : ""}
                {play.res.net} {currency}
              </div>
              <div className="mo-res-row">
                <span>{tm.hitsLabel} <b className="num">
                  {play.res.hits}
                </b></span>
                <span>{tm.bombsLabel} <b className="num">
                  {play.res.bombs}
                </b></span>
                <span>{tm.bestLabel} <b className="num">
                  {play.res.best}
                </b></span>
              </div>
              <div className="mo-res-cta">
                <button
                  type="button"
                  className="arc-chip on"
                  onClick={() => void play.start()}
                >
                  {tm.again}
                </button>
                <Link href="/games" className="arc-chip">
                  {tm.backHall}
                </Link>
              </div>
            </div>
          )}
        </div>
        {!play.res && (
          <div className="mo-cta">
            <button
              type="button"
              className="arc-chip on"
              disabled={playing || play.busy || limitLeft <= 0}
              onClick={() => void play.start()}
            >
              {playing ? tm.playing : `${tm.go} · ${ticket}`}
            </button>
            <button
              type="button"
              className={`arc-chip auto${play.auto ? " on" : ""}`}
              onClick={play.toggleAuto}
            >
              {play.auto ? tm.autoStop : tm.autoGo}
            </button>
          </div>
        )}
        <div className="wp-mid">
          <StreakPill n={streak} label={t.streakLabel} />
          {started && !play.res && (
            <span className="mo-live num">
              {tm.hitsLabel} {play.hits} · {tm.bombsLabel} {play.bombs}
            </span>
          )}
        </div>
        <div className="wp-stats">
          <div className="wp-stat">
            <div className="wp-stat-lb">{tm.statLeft}</div>
            <div className="wp-stat-vl num">{limitLeft}</div>
          </div>
          <div className="wp-stat">
            <div className="wp-stat-lb">{tm.statMax}</div>
            <div className="wp-stat-vl num gold">
              {maxMult > 0 ? `${maxMult}x` : "—"}
            </div>
          </div>
          <div className="wp-stat">
            <div className="wp-stat-lb">{tm.statNet}</div>
            <div className={`wp-stat-vl num ${todayNet >= 0 ? "green" : ""}`}>
              {todayNet > 0 ? "+" : ""}{todayNet}
            </div>
          </div>
        </div>
        {notice && <p className="wp-notice">{notice}</p>}
        {play.err && (
          <p role="alert" className="wp-error">{play.err}</p>
        )}
      </div>
    );

  return (
    // 不要带 wheel-page：大转盘皮肤有 33 条 .wheel-page 作用域规则（含把舞台
    // 染成深紫的 !important），会压过打地鼠自己的样式——此前打地鼠的舞台
    // 一直是被大转盘的规则染色的。标题图标的颜色已在 arcade-mole.css 自带。
    <div className={`mole-page tab-${tab}`}>
      <GameShell
        icon={<ArcadeGlyph k="mole" />}
        title={tm.title}
        subtitle={tm.sub}
        headerLayout="center"
        badges={
          <>
            <span className="wp-badge">
              {tm.roundBadge.replace("{n}", String(spawns + bombSpawns))}
            </span>
            <span className="wp-badge ticket">
              {tm.ticketBadge.replace("{n}", String(ticket))}
            </span>
          </>
        }
        balance={ov?.me?.balance ?? null}
        todayNet={ov?.me?.today_net ?? null}
        limitLeft={ov?.me?.limit_left ?? null}
        balanceVariant="chips"
        balanceLabel={tm.balance}
        limitLabel={tm.statLeft}
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
