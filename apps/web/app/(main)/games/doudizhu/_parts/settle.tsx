"use client";
import { useI18n } from "@/i18n/client";
import type { DzGame } from "./room-types";
import { ReplayPanel } from "./replay";

/** 结算浮层：胜/负/流局 + 再来一局（回大厅重新选档）。 */
export function SettlePanel({
  game,
  onAgain,
}: {
  game: DzGame;
  onAgain: () => void;
}) {
  const { dict, currency } = useI18n();
  const t = dict.games.doudizhu;
  const magic = currency || t.magicUnit;
  return (
    <div className="dz-settle" role="status">
      <h2 className="dz-h2">{t.gameDone}</h2>
      {game.result === "win" && (
        <p className="is-win">
          {t.settleWin
            .replace("{net}", String(game.net))
            .replace("{magic}", magic)}
        </p>
      )}
      {game.result === "lose" && (
        <p className="is-lose">
          {t.settleLose.replace("{stake}", String(game.stake))}
        </p>
      )}
      {game.result === null && <p>{t.settleDraw}</p>}
      <ReplayPanel events={game.events ?? game.events_tail} mySeat={0} />
      <div className="dz-controls">
        <button
          type="button"
          className="dz-btn is-primary"
          onClick={onAgain}
        >
          {t.againBtn}
        </button>
      </div>
    </div>
  );
}
