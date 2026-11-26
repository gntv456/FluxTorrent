"use client";
import { useEffect } from "react";
import { useI18n } from "@/i18n/client";
import type { ComboResult } from "./combo";
import type { DzGame, Seat } from "./room-types";
import { sortHand } from "./room-types";
import { CountdownBar } from "./countdown";
import { DzTable } from "./table";
import { Hand } from "./cards";
import { DzControls } from "./controls";
import { DdCounter } from "./counter";

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
        <button type="button" className="dz-btn dz-exit" onClick={onExit}>
          ← {t.backHall}
        </button>
        <div className="dz-status dz-fullstatus">
          <span className="dz-status-stake">
            {t.stakeTier.replace("{n}", String(game.stake))}
          </span>
          <span>{t.mult.replace("{n}", String(game.multiplier))}</span>
          {game.bomb_count > 0 && (
            <span className="dz-status-bomb">
              {t.bombs.replace("{n}", String(game.bomb_count))}
            </span>
          )}
          {game.landlord !== null && (
            <span>
              {(game.landlord === 0 ? t.seatYou : t.seatLandlord) +
                (game.landlord === 0
                  ? ""
                  : " · " + t.aiName.replace("{n}", String(game.landlord)))}
            </span>
          )}
        </div>
        <span className="dz-fullbalance">{balance ?? "—"}</span>
      </div>
      <CountdownBar
        game={game}
        seconds={turnSeconds}
        onExpire={onExpire}
      />
      <DzTable game={game} full />
      {me && (game.phase === "playing" || game.phase === "discard") && (
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
      {game.phase === "playing" && <DdCounter game={game} me={me} />}
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
    </div>
  );
}
