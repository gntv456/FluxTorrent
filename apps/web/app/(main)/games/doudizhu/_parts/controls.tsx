"use client";
import { useI18n } from "@/i18n/client";
import type { DzGame } from "./room-types";
import type { ComboKind, ComboResult } from "./combo";

/** 牌型 → 三语文案键（识别器仅用于选牌阶段软提示，合法性仍以后端为准）。 */
type DzDict = ReturnType<typeof useI18n>["dict"]["games"]["doudizhu"];
export const COMBO_KEY: Record<ComboKind, keyof DzDict> = {
  single: "comboSingle",
  pair: "comboPair",
  triple: "comboTriple",
  tripleOne: "comboTripleOne",
  triplePair: "comboTriplePair",
  straight: "comboStraight",
  doublePair: "comboDoublePair",
  plane: "comboPlane",
  bomb: "comboBomb",
  rocket: "comboRocket",
  fourTwo: "comboFourTwo",
  invalid: "comboInvalid",
};

/** 控制区：叫分条（bidding）或 出牌/不出/提示（playing·我方轮）。 */
export function DzControls({
  game,
  hintBid,
  selectedCount,
  combo,
  busy,
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
  hintBid: number | null;
  selectedCount: number;
  combo: ComboResult | null;
  busy: boolean;
  onBid: (score: number) => void;
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
            aria-keyshortcuts={String(s)}
            onClick={() => onBid(s)}
          >
            {bidLabel(s)}
          </button>
        ))}
      </div>
    );
  }
  if (game.phase === "grab") {
    return (
      <div className="dz-controls" role="group" aria-label={t.grabTitle}>
        <button
          type="button"
          className="dz-btn"
          disabled={busy || !game.my_turn}
          onClick={() => onGrab(true)}
        >
          {t.grabYes}
        </button>
        <button
          type="button"
          className="dz-btn"
          disabled={busy || !game.my_turn}
          onClick={() => onGrab(false)}
        >
          {t.grabNo}
        </button>
      </div>
    );
  }
  if (game.phase === "discard") {
    const invalid = selectedCount > 3;
    return (
      <div className="dz-controls" role="group" aria-label={t.discardTitle}>
        <span className="dz-combo-count">{t.discardTip}</span>
        <button
          type="button"
          className="dz-btn"
          disabled={busy}
          onClick={onKeep}
        >
          {t.discardKeep}
        </button>
        <button
          type="button"
          className="dz-btn is-primary"
          disabled={busy || invalid}
          onClick={() => onDiscard()}
        >
          {t.discardBtn}
        </button>
      </div>
    );
  }
  if (game.phase === "double") {
    return (
      <div className="dz-controls" role="group" aria-label={t.doubleTitle}>
        <button
          type="button"
          className="dz-btn"
          disabled={busy || !game.my_turn}
          onClick={() => onDouble(0)}
        >
          {t.double0}
        </button>
        <button
          type="button"
          className="dz-btn"
          disabled={busy || !game.my_turn}
          onClick={() => onDouble(1)}
        >
          {t.double1}
        </button>
        <button
          type="button"
          className="dz-btn is-primary"
          disabled={busy || !game.my_turn}
          onClick={() => onDouble(2)}
        >
          {t.double2}
        </button>
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
  const invalid = combo != null && !combo.valid;
  const canPlay = !busy && selectedCount > 0 && combo?.valid !== false;
  return (
    <div className="dz-controls" role="group" aria-label={t.playTitle}>
      {selectedCount > 0 && combo && (
        <span
          className={`dz-combo${invalid ? " is-invalid" : ""}`}
          aria-live="polite"
        >
          {t[COMBO_KEY[combo.kind]]}
        </span>
      )}
      {selectedCount > 0 && (
        <span className="dz-combo-count">
          {t.selectedCount.replace("{n}", String(selectedCount))}
        </span>
      )}
      <button
        type="button"
        className="dz-btn"
        disabled={busy}
        aria-keyshortcuts="h"
        title="H"
        onClick={onHint}
      >
        {t.hintBtn}
      </button>
      <button
        type="button"
        className="dz-btn"
        disabled={busy || mustLead}
        aria-keyshortcuts="Space"
        title="Space"
        onClick={onPass}
      >
        {t.passBtn}
      </button>
      <button
        type="button"
        className="dz-btn is-primary"
        disabled={!canPlay}
        aria-keyshortcuts="Enter"
        title="Enter"
        onClick={onPlay}
      >
        {t.playBtn}
      </button>
    </div>
  );
}
