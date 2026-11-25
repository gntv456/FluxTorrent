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

/** 座位最新台词（叫分/不出/出牌张数），模仿开源 landlord 的 say() 气泡。 */
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
    if (e.cards?.length) return String(e.cards.length);
    return null;
  }
  return null;
}

/** 牌桌舞台：三座（身份头像+气泡+剩牌徽）+ 中央落牌区 + 底牌 + 状态行。 */
export function DzTable({ game }: { game: DzGame }) {
  const { dict } = useI18n();
  const t = dict.games.doudizhu;
  const seats = [1, 2].map((s) => game.seats.find((x) => x.seat === s));
  const me = game.seats.find((x) => x.seat === 0);
  const revealed = game.landlord !== null;
  return (
    <div className="dz-table">
      <div className="dz-seats">
        {seats.map(
          (s) =>
            s && (
              <div
                key={s.seat}
                className={`dz-seat${game.turn === s.seat ? " is-turn" : ""}`}
              >
                <div
                  className={`dz-seat-avatar is-${
                    s.role === "landlord"
                      ? "landlord"
                      : s.is_ai
                        ? "ai"
                        : "farmer"
                  }`}
                  aria-hidden
                >
                  {s.role === "landlord" ? "🎩" : s.is_ai ? "🤖" : "🧑"}
                </div>
                <div className="dz-seat-name">
                  {seatName(t, s.seat)}
                  {s.role && (
                    <Badge
                      tone={s.role === "landlord" ? "danger" : "neutral"}
                      size="sm"
                      className="dz-seat-role"
                    >
                      {s.role === "landlord" ? t.seatLandlord : t.seatFarmer}
                    </Badge>
                  )}
                </div>
                <div className="dz-seat-count">
                  {t.cardsLeft.replace("{n}", String(s.count))}
                </div>
                {game.phase === "bidding" && (
                  <div className="dz-seat-bid">
                    {(() => {
                      const bid = game.bids.find((b) => b[0] === s.seat);
                      return bid
                        ? bid[1] > 0
                          ? t.scoreUnit.replace("{n}", String(bid[1]))
                          : t.bid0
                        : t.bidWait;
                    })()}
                  </div>
                )}
                {(() => {
                  const bubble = seatBubble(game, s.seat, t);
                  return bubble ? (
                    <div className="dz-bubble">{bubble}</div>
                  ) : null;
                })()}
              </div>
            ),
        )}
      </div>

      <div className="dz-center">
        <div className="dz-status">
          <span>
            {t.mult.replace("{n}", String(game.multiplier))}
          </span>
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
            <div className="dz-played">
              {game.last.cards.map((c, i) => (
                <div
                  key={c}
                  className="dz-played-card"
                  style={{ animationDelay: `${i * 60}ms` }}
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
