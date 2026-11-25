"use client";
import { useI18n } from "@/i18n/client";
import { api } from "@/lib/api-client";

export interface PvpRoom {
  id: number;
  variant: string;
  stake: number;
  status: string;
  is_owner: boolean;
  my_seat: number | null;
  seats: { seat: number; filled: boolean; is_me: boolean }[];
  empty_seats: number[];
}

export interface PvpRoomsResp {
  rooms: {
    id: number;
    variant: string;
    stake: number;
    players: number;
    owner: boolean;
  }[];
  mine: PvpRoom | null;
  stake_tiers: number[];
}

/** PvP 房间等待面板：模式选择 + 快速匹配 + 座位图 + 房主开局。 */
export function PvpLobby({
  mode,
  stakeTiers,
  busy,
  onQuickMatch,
  onLeave,
  onDeal,
}: {
  mode: "classic" | "wild";
  stakeTiers: number[];
  busy: boolean;
  onQuickMatch: (stake: number) => void;
  onLeave: () => void;
  onDeal: () => void;
}) {
  const { dict } = useI18n();
  const t = dict.games.doudizhu;
  return (
    <div className="dz-lobby">
      <h2 className="dz-h2">{t.pvpTab}</h2>
      <p className="dz-rule-line">
        {mode === "wild" ? t.wildTip : t.rule}
      </p>
      <div className="dz-tiers">
        {stakeTiers.map((s) => (
          <button
            key={s}
            type="button"
            className="dz-tier"
            disabled={busy}
            onClick={() => onQuickMatch(s)}
          >
            <strong>{t.stakeTier.replace("{n}", String(s))}</strong>
            <span>{t.quickMatch}</span>
          </button>
        ))}
      </div>
      <div className="dz-controls" style={{ marginTop: 12 }}>
        <button
          type="button"
          className="dz-btn"
          disabled={busy}
          onClick={onLeave}
        >
          {t.leaveRoom}
        </button>
      </div>
      <div className="dz-controls" style={{ marginTop: 8 }}>
        <button
          type="button"
          className="dz-btn is-primary"
          disabled={busy}
          onClick={onDeal}
        >
          {t.dealBtn}
        </button>
      </div>
      <p className="dz-rule-line" style={{ marginTop: 8 }}>
        {t.needThree}
      </p>
    </div>
  );
}

/** 拉取我的 PvP 房间（含座位图状态）。 */
export async function fetchRooms(): Promise<PvpRoomsResp | null> {
  return api
    .get<PvpRoomsResp>("/api/v1/games/doudizhu/pvp/rooms")
    .catch(() => null);
}

/** 房间座位等待视图。 */
export function RoomWait({ room }: { room: PvpRoom }) {
  const { dict } = useI18n();
  const t = dict.games.doudizhu;
  const filled = room.seats.length;
  return (
    <div className="dz-settle">
      <h2 className="dz-h2">{t.roomWaiting}</h2>
      <p className="dz-rule-line">
        {t.waitingPlayers.replace("{n}", String(filled))}
      </p>
      <div className="dz-seats">
        {[0, 1, 2].map((seat) => {
          const s = room.seats.find((x) => x.seat === seat);
          return (
            <div key={seat} className="dz-seat">
              <div className="dz-seat-avatar" aria-hidden>
                {s ? "🧑" : "⏳"}
              </div>
              <div className="dz-seat-name">
                {s ? (s.is_me ? t.you : "P" + (seat + 1)) : "…"}
              </div>
            </div>
          );
        })}
      </div>
    </div>
  );
}
