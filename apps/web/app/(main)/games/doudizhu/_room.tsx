"use client";
import { useCallback, useEffect, useState } from "react";
import { api } from "@/lib/api-client";
import { apiErrorMessage, useI18n } from "@/i18n/client";
import { GameShell } from "@/components/game/game-kit";
import { GameTabs } from "@/components/game/game-tabs";
import { GameRecordsPanel } from "@/components/game/game-records";
import { useRounds } from "@/components/game/use-rounds";
import { clientUuid } from "@/lib/client-uuid";
import { useSfx } from "@/components/game/use-sfx";
import type { SfxCue } from "@/lib/sfx";
import { DzTable } from "./_parts/table";
import { DzControls } from "./_parts/controls";
import { Hand } from "./_parts/cards";
import { SettlePanel } from "./_parts/settle";
import { CountdownBar } from "./_parts/countdown";
import { AiLobby, ModeSwitch } from "./_parts/ai-lobby";
import { PvpLobby, RoomWait, fetchRooms } from "./_parts/pvp-lobby";
import {
  sortHand,
  type DzGame,
  type HintResp,
  type Overview,
  type StateResp,
} from "./_parts/room-types";/** 按对局结果与动作选音效线索：终局按输赢；
 *  过牌 pass、叫分 deal、出牌 pop（资深玩家听声辨动作）。 */
function cueOf(
  done: boolean,
  win: boolean,
  action: string,
  passed: boolean,
): SfxCue {
  if (done) return win ? "win" : "lose";
  if (passed) return "pass";
  return action === "play" ? "pop" : "deal";
}

/** 斗地主专注页：AI 房即开即玩 / PvP 房间匹配 → 叫分 → 出牌 → 结算。 */
export default function DoudizhuRoom({
  initialOver,
}: {
  initialOver: Overview | null;
}) {
  const { dict } = useI18n();
  const t = dict.games.doudizhu;
  const [ov, setOv] = useState<Overview | null>(initialOver);
  const [st, setSt] = useState<StateResp | null>(null);
  const [sel, setSel] = useState<Set<number>>(new Set());
  const [busy, setBusy] = useState(false);
  const [err, setErr] = useState<string | null>(null);
  const [tab, setTab] = useState("play");
  const [mode, setMode] = useState<"ai" | "pvp">("ai");
  const [hintBid, setHintBid] = useState<number | null>(null);
  const [variant, setVariant] = useState<"classic" | "wild">("classic");
  const [pvpRooms, setPvpRooms] = useState<Awaited<
    ReturnType<typeof fetchRooms>
  >>(null);
  const { reload: reloadRounds } = useRounds("doudizhu", 20);
  const sfx = useSfx();

  const loadState = useCallback(async () => {
    try {
      setSt(await api.get<StateResp>("/api/v1/games/doudizhu/state"));
    } catch {
      /* 状态拉取失败不阻断页面 */
    }
  }, []);

  const loadMeta = useCallback(async () => {
    try {
      setOv(await api.get<Overview>("/api/v1/games"));
    } catch {
      /* ignore */
    }
    void reloadRounds();
  }, [reloadRounds]);

  const loadRooms = useCallback(
    async () => setPvpRooms(await fetchRooms()),
    [],
  );

  useEffect(() => {
    void loadMeta();
    void loadState();
    void loadRooms();
  }, [loadMeta, loadState, loadRooms]);

  const hasActiveGame = st?.game != null; // PvP 等待轮询闸

  useEffect(() => {
    if (hasActiveGame || mode !== "pvp") return;
    const tick = window.setInterval(() => void loadRooms(), 4000);
    return () => window.clearInterval(tick);
  }, [hasActiveGame, mode, loadRooms]);
  const game = st?.game ?? null;
  const me = game?.seats[0]; const turnSeconds = st?.turn_seconds ?? 20;
  const toggle = (c: number) =>
    setSel((cur) => {
      const next = new Set(cur);
      void (next.has(c) ? next.delete(c) : next.add(c));
      return next;
    });

  // busy/err 统一管线：四个动作共用。
  const run = async (fn: () => Promise<void>) => {
    if (busy) return;
    setBusy(true);
    setErr(null);
    try {
      await fn();
    } catch (e) {
      setErr(apiErrorMessage(dict, e));
    } finally {
      setBusy(false);
    }
  };
  function start(stake: number) {
    void run(async () => {
      const r = await api.post<{ game: DzGame }>(
        "/api/v1/games/doudizhu/start",
        { stake, variant, idempotency_key: clientUuid() },
      );
      setSt((cur) => (cur ? { ...cur, game: r.game } : cur));
      setSel(new Set());
      void loadMeta();
    });
  }
  function act(action: "bid" | "play", score?: number, cards?: number[]) {
    void run(async () => {
      const r = await api.post<{ game: DzGame }>(
        "/api/v1/games/doudizhu/action",
        {
          action,
          ...(action === "bid" ? { score } : { cards: cards ?? [...sel] }),
          idempotency_key: clientUuid(),
        },
      );
      const done = r.game.phase === "done";
      const passed = action === "play" && sel.size === 0;
      sfx.play(cueOf(done, r.game.result === "win", action, passed));
      setSt((cur) => (cur ? { ...cur, game: r.game } : cur));
      setSel(new Set());
      setHintBid(null); // 建议已消费/过期
      void loadMeta();
    });
  }

  function hint() {
    void run(async () => {
      const r = await api.post<HintResp>(
        "/api/v1/games/doudizhu/action",
        { action: "hint" },
      );
      if (r.hintBid !== undefined) setHintBid(r.hintBid); // 显示浮签
      else if (r.hint) setSel(new Set(r.hint));
    });
  }
  /** PvP 动作封装：join/leave/deal 同一错误与 busy 管线。 */
  function pvp(verb: "join" | "leave" | "deal", stake?: number) {
    void run(async () => {
      const body: Record<string, unknown> =
        verb === "join" ? { stake, variant } : {};
      const r = await api.post<{ game?: DzGame | null }>(
        `/api/v1/games/doudizhu/pvp/${verb}`,
        body,
      );
      const g = r.game ?? null;
      if (g?.phase === "done")
        sfx.play(cueOf(true, g.result === "win", "play", false));
      if (g) setSt((cur) => (cur ? { ...cur, game: g } : cur));
      await loadRooms();
    });
  }

  const balance = ov?.me?.balance ?? null;
  const todayNet = ov?.me?.today_net ?? null;
  const modeSwitch = (
    <ModeSwitch
      mode={mode}
      variant={variant}
      busy={busy}
      onMode={setMode}
      onVariant={setVariant}
    />
  );

  return (
    <GameShell
      icon="🃏"
      title={t.title}
      subtitle={t.sub}
      balance={balance}
      todayNet={todayNet}
      limitLeft={ov?.me?.limit_left ?? null}
      headerLayout="center"
      stage={
        tab === "play" && (
          <div className="doudizhu-page">
            {err && (
              <p className="dz-error" role="alert">
                {err}
              </p>
            )}
            {!game && st && (
              <div className="dz-lobby">
                {modeSwitch}
                {mode === "ai" ? (
                  <AiLobby
                    ruleLine={variant === "wild" ? t.wildTip : t.rule}
                    stakeTiers={st.stake_tiers ?? [100, 500, 2000]}
                    variant={variant}
                    busy={busy}
                    onVariant={setVariant}
                    onGoPvp={() => setMode("pvp")}
                    onStart={(s) => void start(s)}
                  />
                ) : pvpRooms?.mine ? (
                  <RoomWait room={pvpRooms.mine} />
                ) : (
                  <PvpLobby
                    mode={variant}
                    stakeTiers={pvpRooms?.stake_tiers ?? [100, 500, 2000]}
                    busy={busy}
                    onQuickMatch={(s) => void pvp("join", s)}
                    onLeave={() => void pvp("leave")}
                    onDeal={() => void pvp("deal")}
                  />
                )}
              </div>
            )}
            {game && (
              <div className="dz-game">
                {game.phase === "done" && (
                  <SettlePanel
                    game={game}
                    onAgain={() =>
                      void loadState().then(() =>
                        setSt((c) => (c ? { ...c, game: null } : c)),
                      )
                    }
                  />
                )}
                {game.phase !== "done" && (
                  <>
                    <CountdownBar
                      game={game}
                      seconds={turnSeconds}
                      onExpire={() => {
                        if (game.phase === "bidding") return void act("bid", 0);
                        const lead = !game.last || game.last.seat === 0;
                        const min = me?.cards.length
                          ? sortHand(me.cards).at(-1)
                          : undefined;
                        const cards = lead && min !== undefined ? [min] : [];
                        void act("play", undefined, cards);
                      }}
                    />
                    <DzTable game={game} />
                    {me && game.phase === "playing" && (
                      <Hand
                        cards={sortHand(me.cards)}
                        selected={sel}
                        wildcards={me.wildcards}
                        onToggle={toggle}
                      />
                    )}
                    <DzControls
                      game={game}
                      hintBid={hintBid}
                      selectedCount={sel.size}
                      busy={busy}
                      onBid={(s) => void act("bid", s)}
                      onPlay={() => void act("play")}
                      onPass={() => void act("play")}
                      onHint={() => void hint()}
                    />
                  </>
                )}
              </div>
            )}
          </div>
        )
      }
      tabBar={
        <GameTabs
          tabs={[
            { key: "play", label: t.title },
            { key: "records", label: t.records },
          ]}
          active={tab}
          onChange={setTab}
        />
      }
      side={
        tab === "records" ? <GameRecordsPanel game="doudizhu" /> : null
      }
    />
  );
}
