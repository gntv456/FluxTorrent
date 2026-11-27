"use client";
import { cardColor, cardLabel, cardSuit } from "../_parts/room-types";
import { useI18n } from "@/i18n/client";

/** 数字牌 pips 网格位置（3×3，[列,行]）；rank 0..6 = 3..9，10 与人头牌走中央单花色。 */
const PIP_POS: [number, number][][] = [
  [[0, 0], [1, 1], [2, 2]], // 3
  [[0, 0], [2, 0], [0, 2], [2, 2]], // 4
  [[0, 0], [2, 0], [1, 1], [0, 2], [2, 2]], // 5
  [[0, 0], [2, 0], [0, 1], [2, 1], [0, 2], [2, 2]], // 6
  [[0, 0], [1, 0], [2, 0], [0, 1], [2, 1], [0, 2], [2, 2]], // 7
  [[0, 0], [1, 0], [2, 0], [0, 1], [2, 1], [0, 2], [1, 2], [2, 2]], // 8
  [[0, 0], [1, 0], [2, 0], [0, 1], [1, 1], [2, 1], [0, 2], [1, 2], [2, 2]], // 9
];

/** 一张扑克牌（DOM+CSS，无图片资产）：左上/右下双角标；数字牌 pips 阵列，
 *  人头牌/10/王走中央单花色。选中态由父层控制。 */
export function PlayingCard({
  card,
  selected,
  invalid,
  onClick,
  small,
  wild,
}: {
  card: number;
  selected?: boolean;
  /** 选中组牌型非法：红边警示（仅选中态叠加）。 */
  invalid?: boolean;
  onClick?: () => void;
  small?: boolean;
  wild?: boolean;
}) {
  const color = cardColor(card);
  const label = cardLabel(card);
  const suit = cardSuit(card);
  const isJoker = color === "joker";
  const rank = card >= 52 ? 13 : Math.floor(card / 4);
  const pipPositions =
    !small && !isJoker && rank <= 6 ? PIP_POS[rank] : null;
  const cls = [
    "dz-card",
    color === "red" ? "is-red" : isJoker ? "is-joker" : "",
    selected ? "is-sel" : "",
    selected && invalid ? "is-bad" : "",
    small ? "is-small" : "",
    onClick ? "is-pick" : "",
    wild ? "is-wild" : "",
  ]
    .filter(Boolean)
    .join(" ");
  const jokerCorner = card === 53 ? "大" : "小";
  const jokerImg = isJoker
    ? card === 53
      ? "/games/ddz/ddz-joker-big.webp"
      : "/games/ddz/ddz-joker-small.webp"
    : null;
  // 人头牌 J/Q/K（rank 9/10/11）成套中式人物位图；A 与数字牌仍走花色降级。
  const FACE_RANK: Record<number, string> = { 9: "j", 10: "q", 11: "k" };
  const FACE_SUIT = ["spade", "heart", "diamond", "club"];
  const faceImg =
    !isJoker && FACE_RANK[rank]
      ? `/games/ddz/ddz-face-${FACE_RANK[rank]}-${FACE_SUIT[card % 4]}.webp`
      : null;
  return (
    <button
      type="button"
      className={cls}
      onClick={onClick}
      aria-pressed={!!selected}
    >
      <span className="dz-card-corner">
        <span className="dz-card-rank">{isJoker ? jokerCorner : label}</span>
        {!isJoker && (
          <span className="dz-card-suit" aria-hidden>
            {suit}
          </span>
        )}
      </span>
      {pipPositions ? (
        <span className="dz-card-pips" aria-hidden>
          {pipPositions.map(([c, r], i) => (
            <i key={i} style={{ gridColumn: c + 1, gridRow: r + 1 }}>
              {suit}
            </i>
          ))}
        </span>
      ) : (
        !small && (
          <span className="dz-card-pip" aria-hidden>
            {jokerImg && (
              <img className="dz-card-joker-img" src={jokerImg} alt="" />
            )}
            {faceImg && (
              <img className="dz-card-face-img" src={faceImg} alt="" />
            )}
            {isJoker ? (card === 53 ? "★" : "☆") : suit}
          </span>
        )
      )}
      <span className="dz-card-corner is-br">
        <span className="dz-card-rank">{isJoker ? jokerCorner : label}</span>
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
  invalidSel = false,
  onToggle,
  wildcards = [],
}: {
  cards: number[];
  selected: Set<number>;
  /** 当前选中组是否为非法牌型：非法时选中牌红边警示。 */
  invalidSel?: boolean;
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
            invalid={invalidSel}
            wild={wildcards.includes(c)}
            onClick={() => onToggle(c)}
          />
        </div>
      ))}
    </div>
  );
}
