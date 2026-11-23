"use client";
import { useCallback, useEffect, useRef, useState } from "react";
import { api, ApiError, isRetryableNetError } from "@/lib/api-client";
import { apiErrorMessage, useI18n } from "@/i18n/client";
import { fmtMult } from "@/lib/games";
import { GameShell, PlayHint } from "@/components/game/game-kit";
import { GameTabs } from "@/components/game/game-tabs";
import { SideTabs } from "@/components/game/side-tabs";
import { GameRecordsPanel } from "@/components/game/game-records";
import { ArcadeGlyph } from "@/components/arcade/arcade-glyph";
import { useRounds } from "@/components/game/use-rounds";
import { SwHist } from "@/components/game/sw-panels";
import { clientUuid } from "@/lib/client-uuid";
import { RoomStage } from "./_parts/room-stage";
import { RoomControls } from "./_parts/room-controls";
import type { Overview, RoomState } from "./_parts/room-types";

export type { Overview } from "./_parts/room-types";

/** 猜大小「倒计时房间」专注页：按期号下注，到点统一开奖；魔力记账，多端复用 GameShell。 */
export default function BigSmallRoom({
  initialOver,
}: {
  initialOver: Overview | null;
}) {
  const { dict } = useI18n();
  const t = dict.games;
  const tg = dict.games.bigsmall;

  const [ov, setOv] = useState<Overview | null>(initialOver);
  const [room, setRoom] = useState<RoomState | null>(null);
  const { hist, streak, reload: reloadRounds } = useRounds("bigsmall", 20);
  const [bet, setBet] = useState(100);
  const [busy, setBusy] = useState(false);
  const [err, setErr] = useState<string | null>(null);
  const [unknown, setUnknown] = useState(false);
  const [tab, setTab] = useState("play");
  const [sessionPlays, setSessionPlays] = useState(0);
  const idem = useRef<string>("");
  const lastSide = useRef<"small" | "big">("small");

  const loadMeta = useCallback(async () => {
    try {
      setOv(await api.get<Overview>("/api/v1/games"));
    } catch {
      /* ignore */
    }
    void reloadRounds();
  }, [reloadRounds]);

  const loadRoom = useCallback(async () => {
    try {
      setRoom(await api.get<RoomState>("/api/v1/games/bigsmall/room"));
    } catch {
      /* 房间状态失败不阻断页面，倒计时回落本地态 */
    }
  }, []);

  useEffect(() => {
    void loadMeta();
    void loadRoom();
  }, [loadMeta, loadRoom]);

  // 本地每秒递减倒计时；到 0 后重新拉房间（懒结算开新期 + 回放上期开奖）。
  useEffect(() => {
    const tick = window.setInterval(() => {
      setRoom((cur) =>
        cur
          ? {
              ...cur,
              round: {
                ...cur.round,
                remaining: Math.max(0, cur.round.remaining - 1),
              },
            }
          : cur,
      );
    }, 1000);
    return () => window.clearInterval(tick);
  }, []);

  useEffect(() => {
    if (room && room.round.remaining <= 0) void loadRoom();
  }, [room, loadRoom]);

  const maxBet = ov?.max_bet ?? 1000;
  const winMult = room?.win_mult ?? ov?.bigsmall?.win_mult ?? 1.9;
  const closed = (room?.round.remaining ?? 0) <= 0;

  async function place(side: "small" | "big", reuse = false) {
    if (busy) return;
    if (!reuse) lastSide.current = side;
    setBusy(true);
    setErr(null);
    setUnknown(false);
    if (!reuse || !idem.current) idem.current = clientUuid();
    try {
      const r = await api.post<{
        round_id: number;
        side: string;
        amount: number;
        remaining: number;
        mine: RoomState["mine"];
      }>("/api/v1/games/bigsmall/room/bet", {
        bet,
        side,
        idempotency_key: idem.current,
      });
      setRoom((cur) =>
        cur
          ? {
              ...cur,
              round: { ...cur.round, remaining: r.remaining },
              mine: r.mine,
              my_total: r.mine.reduce((a, b) => a + b.amount, 0),
            }
          : cur,
      );
      setSessionPlays((n) => n + 1);
      void loadMeta();
    } catch (e) {
      setErr(
        e instanceof ApiError
          ? e.code === 1002
            ? e.message
            : apiErrorMessage(dict, e)
          : dict.common.networkError,
      );
      if (isRetryableNetError(e)) setUnknown(true);
    } finally {
      setBusy(false);
    }
  }

  if (tab === "records") {
    return (
      <div className="bs-room-page flex flex-col gap-4">
        <GameTabs
          tabs={[
            { key: "play", label: dict.games.sub.tabPlay, icon: "🎲" },
            { key: "records", label: dict.games.sub.tabRecords, icon: "📜" },
          ]}
          active={tab}
          onChange={setTab}
        />
        <GameRecordsPanel game="bigsmall" />
      </div>
    );
  }

  return (
    <div className="bs-room-page">
    <GameShell
      icon={<ArcadeGlyph k="bigsmall" />}
      title={tg.title}
      subtitle={`${tg.rule} · ${tg.winMult.replace("{n}", fmtMult(winMult))}`}
      balance={ov?.me?.balance ?? null}
      todayNet={ov?.me?.today_net ?? null}
      limitLeft={ov?.me?.limit_left ?? null}
      /* 布局优化（2026-11-23）：标签条从 notice 槽（独占一行）挪进
         tabBar 槽与余额条同行，头部省一行。 */
      tabBar={
        <GameTabs
          tabs={[
            { key: "play", label: dict.games.sub.tabPlay, icon: "🎲" },
            { key: "records", label: dict.games.sub.tabRecords, icon: "📜" },
          ]}
          active={tab}
          onChange={setTab}
        />
      }
      notice={
        <PlayHint
          sessionPlays={sessionPlays}
          todayNet={ov?.me?.today_net ?? null}
        />
      }
      stage={
        <RoomStage
          room={room}
          winMult={winMult}
          streak={streak}
          todayPlays={ov?.me?.today_plays ?? 0}
          limitLeft={ov?.me?.limit_left ?? null}
          err={err}
          unknown={unknown}
          onRetry={() => void place(lastSide.current, true)}
        />
      }
      controls={
        <RoomControls
          bet={bet}
          onBet={setBet}
          maxBet={maxBet}
          busy={busy}
          closed={closed}
          room={room}
          winMult={winMult}
          myTotal={room?.my_total ?? 0}
          onPlace={(s) => void place(s)}
        />
      }
      side={
        /* 布局优化（2026-11-23）：赔率 / 战绩收成分区标签，侧栏不再
           竖着堆两块把行高撑过舞台。 */
        <SideTabs
          items={[
            {
              key: "odds",
              label: dict.review.bsOddsTitle,
              content: (
                <>
                  <div className="grid grid-cols-1 gap-1 text-xs">
                    <div className="flex justify-between">
                      <span className="text-sub">
                        {tg.bsPickSmall} · {dict.review.bsRangeSmall}
                      </span>
                      <b className="num text-[var(--sky-deep)]">
                        {fmtMult(winMult)}x
                      </b>
                    </div>
                    <div className="flex justify-between">
                      <span className="text-sub">
                        {tg.bsPickBig} · {dict.review.bsRangeBig}
                      </span>
                      <b className="num text-[var(--sky-deep)]">
                        {fmtMult(winMult)}x
                      </b>
                    </div>
                    <div className="flex justify-between">
                      <span className="text-sub">
                        {dict.review.bsRangeTriple}
                      </span>
                      <b className="num text-[var(--danger-text)]">
                        {dict.review.bsTripleLose}
                      </b>
                    </div>
                  </div>
                  <p className="mt-3 text-xs text-sub">{tg.rule}</p>
                </>
              ),
            },
            {
              key: "road",
              label: t.roadmap,
              content: (
                <SwHist
                  rows={hist.slice(0, 12).map((h) => ({
                    t: (h.at ?? "").slice(11, 16) || "—",
                    txt: h.net > 0 ? tg.bsHistWin : tg.bsHistLose,
                    net: h.net,
                  }))}
                  empty={t.historyEmpty}
                />
              ),
            },
          ]}
        />
      }
    />
    </div>
  );
}
