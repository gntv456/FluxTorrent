"use client";
import { cardColor, cardLabel } from "../_parts/room-types";

/** 一张扑克牌（DOM+CSS，无图片资产）。选中态由父层控制。 */
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
  const cls = [
    "dz-card",
    color === "red" ? "is-red" : color === "joker" ? "is-joker" : "",
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
      <span className="dz-card-rank">{label}</span>
      {wild && <span className="dz-card-wild">★</span>}
      {!small && (
        <span className="dz-card-suit" aria-hidden>
          {color === "red" ? "◆" : color === "joker" ? "★" : "♠"}
        </span>
      )}
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
  return (
    <div className="dz-hand" role="listbox" aria-label="hand">
      {cards.map((c) => (
        <PlayingCard
          key={c}
          card={c}
          selected={selected.has(c)}
          wild={wildcards.includes(c)}
          onClick={() => onToggle(c)}
        />
      ))}
    </div>
  );
}
