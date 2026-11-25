"use client";
import { cardColor, cardLabel, cardSuit } from "../_parts/room-types";
import { useI18n } from "@/i18n/client";

/** 一张扑克牌（DOM+CSS，无图片资产）：左上/右下双角标 + 中央花色，开源
 *  landlord 的 poker spritesheet 语义用 DOM 复刻。选中态由父层控制。 */
export function PlayingCard({
  card,
  selected,
  onClick,
  small,
  wild,
}: {
  card: number;
  selected?: boolean;
  onClick?: () => void;
  small?: boolean;
  wild?: boolean;
}) {
  const color = cardColor(card);
  const label = cardLabel(card);
  const suit = cardSuit(card);
  const isJoker = color === "joker";
  const cls = [
    "dz-card",
    color === "red" ? "is-red" : isJoker ? "is-joker" : "",
    selected ? "is-sel" : "",
    small ? "is-small" : "",
    onClick ? "is-pick" : "",
    wild ? "is-wild" : "",
  ]
    .filter(Boolean)
    .join(" ");
  return (
    <button
      type="button"
      className={cls}
      onClick={onClick}
      aria-pressed={!!selected}
    >
      <span className="dz-card-corner">
        <span className="dz-card-rank">{isJoker ? "🃏" : label}</span>
        {!isJoker && (
          <span className="dz-card-suit" aria-hidden>
            {suit}
          </span>
        )}
      </span>
      {!small && (
        <span className="dz-card-pip" aria-hidden>
          {isJoker ? (card === 53 ? "★" : "☆") : suit}
        </span>
      )}
      <span className="dz-card-corner is-br">
        <span className="dz-card-rank">{isJoker ? "🃏" : label}</span>
        {!isJoker && (
          <span className="dz-card-suit" aria-hidden>
            {suit}
          </span>
        )}
      </span>
      {wild && <span className="dz-card-wild">★</span>}
    </button>
  );
}

/** 手牌区：点选切换选中态（已选的排上来一点）。 */
export function Hand({
  cards,
  selected,
  onToggle,
  wildcards = [],
}: {
  cards: number[];
  selected: Set<number>;
  onToggle: (c: number) => void;
  wildcards?: number[];
}) {
  const { dict } = useI18n();
  const t = dict.games.doudizhu;
  return (
    <div className="dz-hand" role="listbox" aria-label={t.handLabel}>
      {cards.map((c, i) => (
        <div
          key={c}
          className="dz-hand-slot"
          style={{ animationDelay: `${Math.min(i * 40, 600)}ms` }}
        >
          <PlayingCard
            key={c}
            card={c}
            selected={selected.has(c)}
            wild={wildcards.includes(c)}
            onClick={() => onToggle(c)}
          />
        </div>
      ))}
    </div>
  );
}
