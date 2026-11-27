"use client";
import { useI18n } from "@/i18n/client";
import { api } from "@/lib/api-client";
import { RoomChat } from "./room-chat";

export interface PvpRoomSeat {
  seat: number;
  filled: boolean;
  is_me: boolean;
  ready: boolean;
  online: boolean;
  /** 昵称（users.username）；空座/注销兜底为空串。 */
  nickname?: string;
  avatar_url?: string | null;
  /** 称号（users.title）。 */
  title?: string | null;
  /** 余额：仅本人座下发，他人座为 null，不亮筹码。 */
  balance?: number | null;
  user_id?: number;
}

export interface PvpRoom {
  id: number;
  variant: string;
  stake: number;
  status: string;
  is_owner: boolean;
  my_seat: number | null;
  /** public=进公开列表；private=仅凭房号进入。 */
  visibility?: "public" | "private";
  /** 观战人数（有效观战行数）。 */
  spectators?: number;
  seats: PvpRoomSeat[];
  empty_seats: number[];
}

/** 段位派生响应（GET /games/doudizhu/rank，只读聚合）。 */
export interface DzRank {
  plays: number;
  wins: number;
  losses: number;
  net: number;
  win_rate: number;
  rank: {
    tier: number;
    name_key: string;
    stars: number;
    progress: number;
    wins: number;
  };
}

/** 段位 name_key → 三语字典键。 */
const RANK_DICT: Record<string, string> = {
  rookie: "rankRookie",
  beginner: "rankBeginner",
  junior: "rankJunior",
  intermediate: "rankIntermediate",
  senior: "rankSenior",
  master: "rankMaster",
  god: "rankGod",
};

export interface PvpRoomsResp {
  rooms: {
    id: number;
    variant: string;
    stake: number;
    status: "waiting" | "playing";
    players: number;
    spectators: number;
    visibility: "public" | "private";
    owner: boolean;
  }[];
  mine: PvpRoom | null;
  stake_tiers: number[];
}

/** 房间频道消息（GET /pvp/messages，时间升序）。 */
export interface PvpMessage {
  id: number;
  sender_id: number;
  nickname: string;
  body: string;
  created_at: string;
}

/** PvP 房间等待面板：模式选择 + 快速匹配 + 公开房间列表（可观战）。 */
export function PvpLobby({
  mode,
  stakeTiers,
  rooms,
  busy,
  onQuickMatch,
  onLeave,
  onDeal,
  onSpectate,
}: {
  mode: "classic" | "wild" | "nolook";
  stakeTiers: number[];
  rooms: PvpRoomsResp["rooms"];
  busy: boolean;
  onQuickMatch: (stake: number) => void;
  onLeave: () => void;
  onDeal: () => void;
  onSpectate: (rid: number) => void;
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
      <div className="dz-room-list">
        {rooms.length === 0 && (
          <p className="dz-rule-line">{t.roomListEmpty}</p>
        )}
        {rooms.map((r) => (
          <div key={r.id} className="dz-room-row">
            <div className="dz-room-row-info">
              <span className="dz-room-row-title">
                {t.roomId.replace("{n}", String(r.id))}
                {r.visibility === "private" ? ` · ${t.privateTag}` : ""}
              </span>
              <span className="dz-room-row-meta">
                {t.stakeTier.replace("{n}", String(r.stake))}
                {" · "}
                {t.waitingPlayers.replace("{n}", String(r.players))}
                {typeof r.spectators === "number" &&
                  ` · ${t.spectators.replace("{n}", String(r.spectators))}`}
                {" · "}
                {r.status === "playing" ? t.roomPlaying : t.roomWaiting}
              </span>
            </div>
            <button
              type="button"
              className="dz-btn"
              disabled={busy}
              onClick={() => onSpectate(r.id)}
            >
              {t.watchBtn}
            </button>
          </div>
        ))}
      </div>
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
export function RoomWait({
  room,
  busy,
  onToggleReady,
  onDeal,
}: {
  room: PvpRoom;
  busy: boolean;
  onToggleReady: () => void;
  onDeal: () => void;
}) {
  const { dict } = useI18n();
  const t = dict.games.doudizhu;
  const filled = room.seats.length;
  const mine = room.seats.find((s) => s.is_me);
  const myReady = mine?.ready ?? false;
  return (
    <div className="dz-settle">
      <h2 className="dz-h2">{t.roomWaiting}</h2>
      <p className="dz-rule-line">
        {t.roomId.replace("{n}", String(room.id))}
        {room.is_owner ? ` · ${t.ownerTag}` : ""}
        {room.visibility === "private" ? ` · ${t.privateTag}` : ""}
      </p>
      <p className="dz-rule-line">
        {t.waitingPlayers.replace("{n}", String(filled))}
        {typeof room.spectators === "number" &&
          ` · ${t.spectators.replace("{n}", String(room.spectators))}`}
      </p>
      <div className="dz-seats dz-wait-seats">
        {[0, 1, 2].map((seat) => {
          const s = room.seats.find((x) => x.seat === seat);
          return (
            <div key={seat} className="dz-seat">
              <div className="dz-seat-avatar" aria-hidden>
                {s ? (
                  <img
                    src={
                      s.avatar_url ||
                      (s.is_me
                        ? "/games/ddz/ddz-farmer.webp"
                        : "/games/ddz/ddz-ai.webp")
                    }
                    alt=""
                    draggable="false"
                  />
                ) : null}
              </div>
              <div className="dz-seat-name">
                {s ? (s.is_me ? t.you : s.nickname || "P" + (seat + 1)) : "…"}
              </div>
              {s?.title && (
                <div className="dz-seat-status-text">{s.title}</div>
              )}
              {s && (
                <div className="dz-seat-status">
                  <span
                    className={`dz-online-dot${s.online ? " is-online" : ""}`}
                    aria-hidden
                  />
                  <span className="dz-seat-status-text">
                    {s.ready ? t.readyTag : t.onlineTag}
                  </span>
                </div>
              )}
              {s && s.is_me && typeof s.balance === "number" && (
                <div className="dz-seat-status-text">
                  {t.myBalance.replace("{n}", String(s.balance))}
                </div>
              )}
            </div>
          );
        })}
      </div>
      <RoomChat roomId={room.id} />
      <div className="dz-controls">
        <button
          type="button"
          className="dz-btn"
          disabled={busy}
          onClick={onToggleReady}
        >
          {myReady ? t.readyCancel : t.readyBtn}
        </button>
        {room.is_owner && (
          <button
            type="button"
            className="dz-btn is-primary"
            disabled={busy}
            onClick={onDeal}
          >
            {t.dealBtn}
          </button>
        )}
      </div>
    </div>
  );
}

/** 段位徽章：称号 + 星级 + 胜率。 */
export function RankBadge({ rank }: { rank: DzRank }) {
  const { dict } = useI18n();
  const t = dict.games.doudizhu;
  const nameKey = RANK_DICT[rank.rank.name_key] ?? "rankRookie";
  const name = t[nameKey as keyof typeof t];
  return (
    <div className="dz-rank" aria-label={t.rankTitle}>
      <span className="dz-rank-name">{name}</span>
      <span className="dz-rank-stars" aria-hidden>
        {"★".repeat(rank.rank.stars)}
        <span className="dz-rank-stars-off">
          {"★".repeat(3 - rank.rank.stars)}
        </span>
      </span>
      <span className="dz-rank-meta">
        {t.rankWinRate.replace("{n}", String(Math.round(rank.win_rate * 100)))}
      </span>
    </div>
  );
}

/** 拉取我的段位派生数据。 */
export async function fetchRank(): Promise<DzRank | null> {
  return api
    .get<DzRank>("/api/v1/games/doudizhu/rank")
    .catch(() => null);
}
