"use client";
import { useI18n } from "@/i18n/client";
import type { DzGame } from "./room-types";

/** 控制区：叫分条（bidding）或 出牌/不出/提示（playing·我方轮）。 */
export function DzControls({
  game,
  hintBid,
  selectedCount,
  busy,
  onBid,
  onPlay,
  onPass,
  onHint,
}: {
  game: DzGame;
  hintBid: number | null;
  selectedCount: number;
  busy: boolean;
  onBid: (score: number) => void;
  onPlay: () => void;
  onPass: () => void;
  onHint: () => void;
}) {
  const { dict } = useI18n();
  const t = dict.games.doudizhu;
  if (game.phase === "bidding") {
    const bidLabel = (s: number) =>
      s === 0 ? t.bid0 : t.bid1.replace("1", String(s));
    return (
      <div className="dz-controls" role="group" aria-label={t.bidTitle}>
        {hintBid !== null && (
          <span className="dz-hint-bid">
            {bidLabel(hintBid)}
          </span>
        )}
        {[0, 1, 2, 3].map((s) => (
          <button
            key={s}
            type="button"
            className="dz-btn"
            disabled={busy || !game.my_turn}
            onClick={() => onBid(s)}
          >
            {bidLabel(s)}
          </button>
        ))}
      </div>
    );
  }
  if (game.phase === "done") return null;
  if (!game.my_turn) {
    return (
      <div className="dz-controls is-wait">
        <span>{t.playWait}</span>
      </div>
    );
  }
  const mustLead =
    !game.last || game.last.seat === 0 || game.last.cards.length === 0;
  return (
    <div className="dz-controls" role="group" aria-label={t.playTitle}>
      <button type="button" className="dz-btn" disabled={busy} onClick={onHint}>
        {t.hintBtn}
      </button>
      <button
        type="button"
        className="dz-btn"
        disabled={busy || mustLead}
        onClick={onPass}
      >
        {t.passBtn}
      </button>
      <button
        type="button"
        className="dz-btn is-primary"
        disabled={busy || selectedCount === 0}
        onClick={onPlay}
      >
        {t.playBtn}
      </button>
    </div>
  );
}
