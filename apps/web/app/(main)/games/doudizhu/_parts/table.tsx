"use client";
import { useI18n } from "@/i18n/client";
import { Badge } from "@/components/ui/badge";
import { cardLabel, type DzGame } from "./room-types";
import { PlayingCard } from "./cards";

function seatName(
  t: ReturnType<typeof useI18n>["dict"]["games"]["doudizhu"],
  seat: number,
) {
  if (seat === 0) return t.you;
  return t.aiName.replace("{n}", String(seat));
}

/** 牌桌舞台：三座（上左 AI + 下玩家）+ 桌面最后一手 + 底牌 + 状态行。 */
export function DzTable({ game }: { game: DzGame }) {
  const { dict } = useI18n();
  const t = dict.games.doudizhu;
  const seats = [1, 2].map((s) => game.seats.find((x) => x.seat === s));
  const me = game.seats.find((x) => x.seat === 0);
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
                <div className="dz-seat-avatar" aria-hidden>
                  {s.is_ai ? "🤖" : "🧑"}
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
            <span>{t.bombs.replace("{n}", String(game.bomb_count))}</span>
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
              {game.last.cards.map((c) => (
                <PlayingCard key={c} card={c} small />
              ))}
            </div>
          ) : (
            <span className="dz-table-empty">
              {game.phase === "bidding" ? t.bidTitle : t.tableEmpty}
            </span>
          )}
        </div>
        {game.landlord !== null && game.kitty.length > 0 && (
          <div className="dz-kitty">
            <span className="dz-kitty-label">{t.kitty}</span>
            {game.kitty.map((c) => (
              <span key={c} className="dz-kitty-card">
                {cardLabel(c)}
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
