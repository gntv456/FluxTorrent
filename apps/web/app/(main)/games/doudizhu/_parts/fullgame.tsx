"use client";
import { useEffect, useState } from "react";
import { useI18n } from "@/i18n/client";
import type { ComboResult } from "./combo";
import type { DzGame, Seat, SeatMeta } from "./room-types";
import { sortHand } from "./room-types";
import { CountdownBar } from "./countdown";
import { DzTable } from "./table";
import { Hand, PlayingCard } from "./cards";
import { DzControls } from "./controls";
import { DdCounter } from "./counter";
import { useSfxToggle } from "@/components/game/use-sfx";

/** 全屏沉浸壳：对局中整页=牌桌。退出=回大厅（game 置空由父层处理）。 */
export function FullGame({
  game,
  me,
  err,
  balance,
  turnSeconds,
  sel,
  selCombo,
  hintBid,
  hintNote,
  busy,
  seatMeta,
  spectating,
  onExit,
  onExpire,
  onToggle,
  onBid,
  onPlay,
  onPass,
  onHint,
  onGrab,
  onDiscard,
  onKeep,
  onDouble,
}: {
  game: DzGame;
  me?: Seat;
  err: string | null;
  balance: number | null;
  turnSeconds: number;
  sel: Set<number>;
  selCombo: ComboResult | null;
  hintBid: number | null;
  /** 提示不可用的行内说明（非错误色）。 */
  hintNote: string | null;
  busy: boolean;
  seatMeta?: SeatMeta[];
  /** 旁观视角：无本人手牌与行动权，隐藏手牌/控件/记牌器。 */
  spectating?: boolean;
  onExit: () => void;
  onExpire: () => void;
  onToggle: (c: number) => void;
  onBid: (s: number) => void;
  onPlay: () => void;
  onPass: () => void;
  onHint: () => void;
  onGrab: (take: boolean) => void;
  onDiscard: () => void;
  onKeep: () => void;
  onDouble: (level: number) => void;
}) {
  const { dict } = useI18n();
  const t = dict.games.doudizhu;
  const spect = spectating === true;
  const sfxToggle = useSfxToggle();
  const [isFs, setIsFs] = useState(false);

  useEffect(() => {
    const onFs = () => setIsFs(!!document.fullscreenElement);
    document.addEventListener("fullscreenchange", onFs);
    return () => document.removeEventListener("fullscreenchange", onFs);
  }, []);
  const toggleFs = () => {
    if (document.fullscreenElement) void document.exitFullscreen();
    else void document.documentElement.requestFullscreen().catch(() => {});
  };
  const statusText =
    game.phase === "playing" ? t.roomPlaying : t.roomWaiting;

  // 键盘快捷键：叫分 0/1/2/3；出牌 Enter · 不出 Space · 提示 H（我方轮、非忙碌）
  useEffect(() => {
    if (busy) return;
    const fromField = (e: KeyboardEvent) => {
      const el = e.target as HTMLElement | null;
      return (
        !!el &&
        (el.tagName === "INPUT" ||
          el.tagName === "TEXTAREA" ||
          el.tagName === "SELECT" ||
          el.isContentEditable)
      );
    };
    // 叫分阶段：0 不叫 / 1·2·3 叫对应分
    if (game.phase === "bidding" && game.my_turn) {
      const onKey = (e: KeyboardEvent) => {
        if (fromField(e)) return;
        if (e.key >= "0" && e.key <= "3") {
          e.preventDefault();
          onBid(Number(e.key));
        }
      };
      window.addEventListener("keydown", onKey);
      return () => window.removeEventListener("keydown", onKey);
    }
    if (game.phase !== "playing" || !game.my_turn) return;
    const mustLead =
      !game.last || game.last.seat === 0 || game.last.cards.length === 0;
    const canPlay = sel.size > 0 && selCombo?.valid !== false;
    const onKey = (e: KeyboardEvent) => {
      if (fromField(e)) return;
      if (e.key === "Enter") {
        if (canPlay) {
          e.preventDefault();
          onPlay();
        }
      } else if (e.key === " ") {
        if (!mustLead) {
          e.preventDefault();
          onPass();
        }
      } else if (e.key === "h" || e.key === "H") {
        e.preventDefault();
        onHint();
      }
    };
    window.addEventListener("keydown", onKey);
    return () => window.removeEventListener("keydown", onKey);
  }, [game, busy, sel, selCombo, onPlay, onPass, onHint, onBid]);
  return (
    <div className="doudizhu-page is-fullgame">
      {err && (
        <p className="dz-error" role="alert">
          {err}
        </p>
      )}
      <div className="dz-fulltop">
        <div className="dz-top-left">
          <span className="dz-top-room">
            {t.title} · {t.roomId.replace("{n}", String(game.id))}
          </span>
          <span
            className={`dz-top-status${game.phase === "playing" ? " is-live" : " is-wait"}`}
          >
            {statusText}
          </span>
          {spect && (
            <span className="dz-top-status is-watch">{t.watchingTag}</span>
          )}
        </div>
        <div className="dz-top-right">
          {game.kitty.length > 0 && (
            <div className="dz-top-kitty" aria-label={t.kitty}>
              {game.kitty.map((k) => (
                <PlayingCard key={k} card={k} small />
              ))}
            </div>
          )}
          <span className="dz-status-mult">
            <img className="dz-ico" src="/games/ddz/ddz-icon-mult.webp" alt="" draggable="false" />
            {t.mult.replace("{n}", String(game.multiplier))}
          </span>
          <span className="dz-status-stake">
            <img className="dz-ico" src="/games/ddz/ddz-icon-stake.webp" alt="" draggable="false" />
            {t.stakeTier.replace("{n}", String(game.stake))}
          </span>
          {game.bomb_count > 0 && (
            <span className="dz-status-bomb">
              <img className="dz-ico" src="/games/ddz/ddz-icon-bomb.webp" alt="" draggable="false" />
              {t.bombs.replace("{n}", String(game.bomb_count))}
            </span>
          )}
          <button
            type="button"
            className="dz-btn dz-top-btn"
            aria-pressed={sfxToggle.on}
            onClick={sfxToggle.toggle}
          >
            {sfxToggle.on ? t.soundOn : t.soundOff}
          </button>
          <button
            type="button"
            className="dz-btn dz-top-btn"
            aria-pressed={isFs}
            onClick={toggleFs}
          >
            {t.fullscreen}
          </button>
          {!spect && (
            <span className="dz-fullbalance">{balance ?? "—"}</span>
          )}
          <button type="button" className="dz-btn dz-exit is-danger" onClick={onExit}>
            {spect ? t.leaveWatch : t.exitRoom}
          </button>
        </div>
      </div>
      <CountdownBar
        game={game}
        seconds={turnSeconds}
        onExpire={onExpire}
      />
      <DzTable game={game} full seatMeta={seatMeta} spectating={spect} />
      {!spect && me && (game.phase === "playing" || game.phase === "discard") && (
        <>
          <div
            className={`dz-me-count dz-fullcount${me.count <= 3 ? " is-low" : ""}`}
          >
            {t.cardsLeft.replace("{n}", String(me.count))}
          </div>
          <Hand
            cards={sortHand(me.cards)}
            selected={sel}
            invalidSel={selCombo != null && !selCombo.valid}
            wildcards={me.wildcards}
            onToggle={onToggle}
          />
        </>
      )}
      {!spect && game.phase === "playing" && (
        <DdCounter game={game} me={me} />
      )}
      {!spect && (
      <DzControls
        game={game}
        hintBid={hintBid}
        hintNote={hintNote}
        selectedCount={sel.size}
        combo={selCombo}
        busy={busy}
        onBid={onBid}
        onPlay={onPlay}
        onPass={onPass}
        onHint={onHint}
        onGrab={onGrab}
        onDiscard={onDiscard}
        onKeep={onKeep}
        onDouble={onDouble}
      />
      )}
    </div>
  );
}
