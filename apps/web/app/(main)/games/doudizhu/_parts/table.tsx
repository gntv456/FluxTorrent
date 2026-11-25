"use client";
import { useI18n } from "@/i18n/client";
import { Badge } from "@/components/ui/badge";
import { cardColor, cardLabel, cardSuit, type DzGame } from "./room-types";
import { PlayingCard } from "./cards";

function seatName(
  t: ReturnType<typeof useI18n>["dict"]["games"]["doudizhu"],
  seat: number,
) {
  if (seat === 0) return t.you;
  return t.aiName.replace("{n}", String(seat));
}

/** 座位最新台词（叫分/不出），模仿开源 landlord 的 say() 气泡。 */
function seatBubble(
  game: DzGame,
  seat: number,
  t: ReturnType<typeof useI18n>["dict"]["games"]["doudizhu"],
): string | null {
  const events = game.events_tail ?? [];
  for (let i = events.length - 1; i >= 0; i--) {
    const e = events[i];
    if (e.seat !== seat) continue;
    if (e.action === "bid")
      return e.score && e.score > 0
        ? t.scoreUnit.replace("{n}", String(e.score))
        : t.bid0;
    if (e.action === "pass") return t.passBtn;
    return null;
  }
  return null;
}

/** AI 牌背扇堆：张数三档密度（商业斗地主的座位牌堆语义），纯 CSS。 */
function CardBackFan({ count }: { count: number }) {
  const n = count === 0 ? 0 : count > 10 ? 3 : count > 4 ? 2 : 1;
  return (
    <div className="dz-fan" aria-hidden>
      {Array.from({ length: n }, (_, i) => (
        <i
          key={i}
          className="dz-fan-back"
          style={{ "--i": i } as React.CSSProperties}
        />
      ))}
      <span className="dz-fan-count">{count}</span>
    </div>
  );
}

/** 牌桌舞台：环形三座（左上/右上 AI + 下方玩家位）+ 中央落牌 + 底牌。 */
export function DzTable({ game }: { game: DzGame }) {
  const { dict } = useI18n();
  const t = dict.games.doudizhu;
  const seatOf = (s: number) => game.seats.find((x) => x.seat === s);
  const me = seatOf(0);
  const revealed = game.landlord !== null;
  const rocket = game.last?.kind === "rocket";
  const fromCls =
    game.last?.seat === 1
      ? "from-left"
      : game.last?.seat === 2
        ? "from-right"
        : "from-me";
  const renderSeat = (s: number) => {
    const seat = seatOf(s);
    if (!seat) return null;
    const bubble = seatBubble(game, s, t);
    const bid =
      game.phase === "bidding"
        ? game.bids.find((b) => b[0] === s)
        : undefined;
    return (
      <div
        className={`dz-seat is-pos-${s}${game.turn === s ? " is-turn" : ""}`}
      >
        <div
          className={`dz-seat-avatar is-${
            seat.role === "landlord"
              ? "landlord"
              : seat.is_ai
                ? "ai"
                : "farmer"
          }`}
          aria-hidden
        >
          {seat.role === "landlord" ? "🎩" : seat.is_ai ? "🤖" : "🧑"}
        </div>
        <div className="dz-seat-name">
          {seatName(t, s)}
          {seat.role && (
            <Badge
              tone={seat.role === "landlord" ? "danger" : "neutral"}
              size="sm"
              className="dz-seat-role"
            >
              {seat.role === "landlord" ? t.seatLandlord : t.seatFarmer}
            </Badge>
          )}
        </div>
        <CardBackFan count={seat.count} />
        {bubble && <div className="dz-bubble">{bubble}</div>}
        {bid !== undefined && (
          <div className="dz-seat-bid">
            {bid[1] > 0
              ? t.scoreUnit.replace("{n}", String(bid[1]))
              : t.bid0}
          </div>
        )}
        {game.phase === "bidding" && bid === undefined && (
          <div className="dz-seat-bid is-wait">{t.bidWait}</div>
        )}
      </div>
    );
  };
  return (
    <div className="dz-table">
      {/* 王炸全屏闪电（last.kind=rocket 时触发一次演出） */}
      {rocket && (
        <div
          className="dz-fx-rocket"
          aria-hidden
          key={game.id + "-" + (game.last?.cards.length ?? 0)}
        />
      )}
      {renderSeat(1)}
      {renderSeat(2)}
      <div className="dz-center">
        <div className="dz-status">
          <span>{t.mult.replace("{n}", String(game.multiplier))}</span>
          {game.bomb_count > 0 && (
            <span className="dz-status-bomb">
              💥 {t.bombs.replace("{n}", String(game.bomb_count))}
            </span>
          )}
          {game.landlord !== null && (
            <span>
              {(game.landlord === 0 ? t.seatYou : t.seatLandlord) +
                " · " +
                seatName(t, game.landlord)}
            </span>
          )}
        </div>
        <div className="dz-table-last" aria-label={t.lastPlay}>
          {game.last && game.last.cards.length > 0 ? (
            <div className={`dz-played ${fromCls}`}>
              {game.last.cards.map((c, i) => (
                <div
                  key={c}
                  className="dz-played-card"
                  style={{ animationDelay: `${i * 70}ms` }}
                >
                  <PlayingCard key={c} card={c} small />
                </div>
              ))}
            </div>
          ) : (
            <span className="dz-table-empty">
              {game.phase === "bidding" ? t.bidTitle : t.tableEmpty}
            </span>
          )}
        </div>
        {revealed && game.kitty.length > 0 && (
          <div className="dz-kitty">
            <span className="dz-kitty-label">{t.kitty}</span>
            {game.kitty.map((c, i) => (
              <span
                key={c}
                className="dz-kitty-card"
                style={{ animationDelay: `${i * 120}ms` }}
              >
                {cardLabel(c)}
                <i className={`dz-kitty-suit is-${cardColor(c)}`} aria-hidden>
                  {cardSuit(c)}
                </i>
              </span>
            ))}
          </div>
        )}
        {me && (
          <div className="dz-me-count">
            {t.cardsLeft.replace("{n}", String(me.count))}
          </div>
        )}
      </div>
    </div>
  );
}
