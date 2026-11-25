"use client";
import { useI18n } from "@/i18n/client";
import type { ComboResult } from "./combo";
import type { DzGame, Seat } from "./room-types";
import { sortHand } from "./room-types";
import { CountdownBar } from "./countdown";
import { DzTable } from "./table";
import { Hand } from "./cards";
import { DzControls } from "./controls";

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
  busy,
  onExit,
  onExpire,
  onToggle,
  onBid,
  onPlay,
  onPass,
  onHint,
}: {
  game: DzGame;
  me?: Seat;
  err: string | null;
  balance: number | null;
  turnSeconds: number;
  sel: Set<number>;
  selCombo: ComboResult | null;
  hintBid: number | null;
  busy: boolean;
  onExit: () => void;
  onExpire: () => void;
  onToggle: (c: number) => void;
  onBid: (s: number) => void;
  onPlay: () => void;
  onPass: () => void;
  onHint: () => void;
}) {
  const { dict } = useI18n();
  const t = dict.games.doudizhu;
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
          <span>{t.mult.replace("{n}", String(game.multiplier))}</span>
          {game.bomb_count > 0 && (
            <span className="dz-status-bomb">
              💥 {t.bombs.replace("{n}", String(game.bomb_count))}
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
        <span className="dz-fullbalance">💎 {balance ?? "—"}</span>
      </div>
      <CountdownBar
        game={game}
        seconds={turnSeconds}
        onExpire={onExpire}
      />
      <DzTable game={game} full />
      {me && game.phase === "playing" && (
        <Hand
          cards={sortHand(me.cards)}
          selected={sel}
          wildcards={me.wildcards}
          onToggle={onToggle}
        />
      )}
      <DzControls
        game={game}
        hintBid={hintBid}
        selectedCount={sel.size}
        combo={selCombo}
        busy={busy}
        onBid={onBid}
        onPlay={onPlay}
        onPass={onPass}
        onHint={onHint}
      />
    </div>
  );
}
