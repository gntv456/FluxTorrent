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
import { dzCueOf } from "./_parts/sfx-cue";
import { DzTable } from "./_parts/table";
import { FullGame } from "./_parts/fullgame";
import { DzControls } from "./_parts/controls";
import { Hand } from "./_parts/cards";
import { identifyCombo } from "./_parts/combo";
import { SettlePanel } from "./_parts/settle";
import { CountdownBar } from "./_parts/countdown";
import { AiLobby, ModeSwitch } from "./_parts/ai-lobby";
import { DzExtras } from "./_parts/extras";
import {
  PvpLobby,
  RoomWait,
  fetchRooms,
  fetchRank,
  RankBadge,
  type DzRank,
  type PvpRoom,
} from "./_parts/pvp-lobby";
import {
  sortHand,
  type DzGame,
  type HintResp,
  type Overview,
  type SeatMeta,
  type StateResp,
} from "./_parts/room-types";

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
  const [booted, setBooted] = useState(false);
  const [err, setErr] = useState<string | null>(null);
  const [tab, setTab] = useState("play");
  const [mode, setMode] = useState<"ai" | "pvp">("ai");
  const [hintBid, setHintBid] = useState<number | null>(null);
  /** 提示不可用时的行内说明（非错误色，自动随下次有效提示清除）。 */
  const [hintNote, setHintNote] = useState<string | null>(null);
  const [variant, setVariant] = useState<"classic" | "wild" | "nolook">(
    "classic",
  );
  const [reveal, setReveal] = useState(false);
  const [pvpRooms, setPvpRooms] = useState<Awaited<
    ReturnType<typeof fetchRooms>
  >>(null);
  /** 观战房座位资料（旁观视角对齐牌桌昵称/头像/称号；无本人余额）。 */
  const [specRoom, setSpecRoom] = useState<PvpRoom | null>(null);
  const [rank, setRank] = useState<DzRank | null>(null);
  const { reload: reloadRounds } = useRounds("doudizhu", 20);
  const sfx = useSfx();

  const loadState = useCallback(async () => {
    try {
      setSt(await api.get<StateResp>("/api/v1/games/doudizhu/state"));
    } catch {
      /* 状态拉取失败不阻断页面 */
    } finally {
      setBooted(true);
    }
  }, []);

  const loadMeta = useCallback(async () => {
    try {
      setOv(await api.get<Overview>("/api/v1/games"));
    } catch {
      /* ignore */
    }
    void reloadRounds();
    fetchRank().then(setRank);
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
  const game = st?.game ?? null;
  const me = game?.seats[0]; const turnSeconds = st?.turn_seconds ?? 20;
  const spectating = st?.spectating === true || game?.spectating === true;
  /** PvP 房间座位资料（昵称/头像/称号）按座位号对齐牌桌；AI 房无 mine 回退立绘。 */
  const metaSeats = pvpRooms?.mine?.seats ?? specRoom?.seats;
  const seatMeta: SeatMeta[] | undefined = metaSeats?.map((s) => ({
    seat: s.seat,
    nickname: s.nickname,
    avatar_url: s.avatar_url,
    title: s.title,
  }));

  useEffect(() => {
    if (hasActiveGame || mode !== "pvp") return;
    const tick = window.setInterval(() => void loadRooms(), 4000);
    return () => window.clearInterval(tick);
  }, [hasActiveGame, mode, loadRooms]);
  // 旁观无本人动作，需定时拉取回显桌面进度。
  useEffect(() => {
    if (!spectating) return;
    const tick = window.setInterval(() => void loadState(), 4000);
    return () => window.clearInterval(tick);
  }, [spectating, loadState]);
  const selCombo = sel.size > 0 ? identifyCombo([...sel]) : null;
  const toggle = (c: number) => {
    sfx.play("select");
    setSel((cur) => {
      const next = new Set(cur);
      void (next.has(c) ? next.delete(c) : next.add(c));
      return next;
    });
  };

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
        { stake, variant, reveal, idempotency_key: clientUuid() },
      );
      setSt((cur) => (cur ? { ...cur, game: r.game } : cur));
      setSel(new Set());
      void loadMeta();
    });
  }
  type Act = "bid" | "grab" | "double" | "play" | "discard";
  function act(action: Act, score?: number, cards?: number[]) {
    void run(async () => {
      const body: Record<string, unknown> = {
        action,
        idempotency_key: clientUuid(),
      };
      if (score !== undefined) body.score = score;
      if (action === "play" || action === "discard") {
        body.cards = cards ?? [...sel];
      }
      const r = await api.post<{ game: DzGame }>(
        "/api/v1/games/doudizhu/action",
        body,
      );
      const done = r.game.phase === "done";
      const win = r.game.result === "win";
      const passed = action === "play" && sel.size === 0;
      const kind = r.game.last?.kind;
      sfx.play(dzCueOf(done, win, action, passed, kind));
      setSt((cur) => (cur ? { ...cur, game: r.game } : cur));
      setSel(new Set());
      setHintBid(null); // 建议已消费/过期
      setHintNote(null);
      void loadMeta();
    });
  }

  const onGrab = (take: boolean) => void act("grab", take ? 1 : 0);
  const onDiscard = () => void act("discard", undefined, [...sel]);
  const onKeep = () => void act("discard", undefined, []);
  const onDouble = (level: number) => void act("double", level);

  function hint() {
    void run(async () => {
      const r = await api.post<HintResp>(
        "/api/v1/games/doudizhu/action",
        { action: "hint" },
      );
      if (r.hintBid !== undefined) setHintBid(r.hintBid); // 显示浮签
      else if (r.hint) setSel(new Set(r.hint));
      // 轮转到别人/阶段切换瞬间点提示：后端给 null。静默会让玩家以为
      // 按钮坏了（2026-11-26 实测反馈），行内状态行说一声，不弹错误色。
      else setHintNote(dict.games.doudizhu.hintNone);
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
        sfx.play(g.result === "win" ? "win" : "lose");
      if (g) setSt((cur) => (cur ? { ...cur, game: g } : cur));
      await loadRooms();
    });
  }
  /** 加入观战：旁观指定公开房；拉回看显局面与房间座位资料。 */
  function spectate(rid: number) {
    void run(async () => {
      await api.post("/api/v1/games/doudizhu/pvp/spectate", {
        room_id: rid,
      });
      const ps = await api
        .get<{ room: PvpRoom | null; game: DzGame | null; spectating: boolean }>(
          "/api/v1/games/doudizhu/pvp/state",
        )
        .catch(() => null);
      setSpecRoom(ps?.room ?? null);
      await loadState();
      await loadRooms();
    });
  }
  /** 退出观战：清旁观局面与座位资料。 */
  function leaveWatch() {
    void run(async () => {
      await api.post("/api/v1/games/doudizhu/pvp/spectate/leave", {});
      setSpecRoom(null);
      setSt((cur) => (cur ? { ...cur, game: null, spectating: false } : cur));
      await loadRooms();
    });
  }
  /** 准备/取消准备（waiting 房）。 */
  function toggleReady() {
    const mine = pvpRooms?.mine;
    const ready = !(mine?.seats.find((s) => s.is_me)?.ready ?? false);
    sfx.play("tick");
    void run(async () => {
      await api.post("/api/v1/games/doudizhu/pvp/ready", { ready });
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

  // 对局中走全屏沉浸壳：牌桌占满视口，手牌/按钮悬浮叠层；
  // 大厅/结算/战绩仍走 GameShell 常规布局。
  if (tab === "play" && game && game.phase !== "done") {
    return (
      <FullGame
        game={game}
        me={me}
        err={err}
        balance={balance}
        turnSeconds={turnSeconds}
        sel={sel}
        selCombo={selCombo}
        hintBid={hintBid}
        hintNote={hintNote}
        busy={busy}
        seatMeta={seatMeta}
        spectating={spectating}
        onExit={() => {
          if (spectating) return leaveWatch();
          setSt((cur) => (cur ? { ...cur, game: null } : cur));
          setSel(new Set());
        }}
        onExpire={() => {
          if (game.phase === "bidding") return void act("bid", 0);
          if (game.phase === "grab") return void act("grab", 0);
          if (game.phase === "discard") return void act("discard", undefined, []);
          if (game.phase === "double") return void act("double", 0);
          // 托管口径（2026-11-26 拍板）：领出出最小单张；跟牌先用后端
          // hint 找压得过的候选——有就出，没有才过。与 PvP 服务端托管
          // （ai_choose）同口径，不再一律 pass。
          const lead = !game.last || game.last.seat === 0;
          const min = me?.cards.length
            ? sortHand(me.cards).at(-1)
            : undefined;
          if (lead) {
            const cards = min !== undefined ? [min] : [];
            return void act("play", undefined, cards);
          }
          // hint 走独立管线：失败/候选为空都落到过牌，绝不空转。
          void (async () => {
            try {
              const r = await api.post<HintResp>(
                "/api/v1/games/doudizhu/action",
                { action: "hint" },
              );
              const cards = r.hint ?? [];
              void act("play", undefined, cards);
            } catch {
              void act("play", undefined, []);
            }
          })();
        }}
        onToggle={toggle}
        onBid={(s) => void act("bid", s)}
        onPlay={() => void act("play")}
        onPass={() => void act("play")}
        onHint={() => void hint()}
        onGrab={onGrab}
        onDiscard={onDiscard}
        onKeep={onKeep}
        onDouble={onDouble}
      />
    );
  }

  return (
    <GameShell
      icon="🃏"
      title={t.title}
      subtitle={t.sub}
      balance={balance}
      todayNet={todayNet}
      limitLeft={ov?.me?.limit_left ?? null}
      headerLayout="center"
      // 牌桌要整页视口高度，不套 gs-stage-box 小盒（默认盒高随内容塌缩）。
      stageBox={false}
      // side 只在战绩 tab 有内容——play/结算 tab 不挂两列栅格，否则
      // 右侧空占 280px 白条（2026-11-27 实测截图）。
      singleColumn={tab !== "records"}
      stage={
        tab === "play" && (
          <div className="doudizhu-page">
            {err && (
              <p className="dz-error" role="alert">
                {err}
              </p>
            )}
            {!game && !booted && (
              <div className="dz-lobby dz-loading">
                <span className="dz-loading-text" role="status">
                  {dict.common?.loading ?? "…"}
                </span>
              </div>
            )}
            {!game && booted && st && (
              <div className="dz-lobby">
                <div className="dz-lobby-hero" aria-hidden>
                  <img
                    src="/games/ddz/ddz-hero.webp"
                    alt=""
                    draggable="false"
                  />
                </div>
                {rank && <RankBadge rank={rank} />}
                {modeSwitch}
                {mode === "ai" && (
                  <label className="dz-reveal">
                    <input
                      type="checkbox"
                      checked={reveal}
                      onChange={(e) => {
                        setReveal(e.target.checked);
                        sfx.play("select");
                      }}
                    />
                    {t.revealTip}
                  </label>
                )}
                {mode === "ai" ? (
                  <AiLobby
                    ruleLine={variant === "wild" ? t.wildTip : t.rule}
                    stakeTiers={st.stake_tiers ?? [100, 500, 2000]}
                    busy={busy}
                    onStart={(s) => void start(s)}
                  />
                ) : pvpRooms?.mine ? (
                  <RoomWait
                    room={pvpRooms.mine}
                    busy={busy}
                    onToggleReady={toggleReady}
                    onDeal={() => void pvp("deal")}
                  />
                ) : (
                  <PvpLobby
                    mode={variant}
                    stakeTiers={pvpRooms?.stake_tiers ?? [100, 500, 2000]}
                    rooms={pvpRooms?.rooms ?? []}
                    busy={busy}
                    onQuickMatch={(s) => void pvp("join", s)}
                    onLeave={() => void pvp("leave")}
                    onDeal={() => void pvp("deal")}
                    onSpectate={(rid) => void spectate(rid)}
                  />
                )}
                <DzExtras />
              </div>
            )}
            {game && game.phase === "done" && (
              <div className="dz-game">
                <SettlePanel
                  game={game}
                  onAgain={() =>
                    void loadState().then(() =>
                      setSt((c) => (c ? { ...c, game: null } : c)),
                    )
                  }
                />
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
