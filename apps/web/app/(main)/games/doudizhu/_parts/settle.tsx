"use client";
import { useI18n } from "@/i18n/client";
import type { DzGame } from "./room-types";
import { ReplayPanel } from "./replay";

/** 金币雨/碎裂演出层：胜利撒币、失败墨滴，纯 CSS 动画（商业结算演出）。 */
function SettleFx({ win }: { win: boolean }) {
  return (
    <div className={`dz-settle-fx ${win ? "is-win" : "is-lose"}`} aria-hidden>
      {Array.from({ length: 12 }, (_, i) => (
        <i
          key={i}
          style={{ "--i": i } as React.CSSProperties}
        >
          {win ? "🪙" : "💧"}
        </i>
      ))}
    </div>
  );
}

/** 结算浮层：胜/负/流局 + 演出 + 再来一局（回大厅重新选档）。 */
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
      {game.result !== null && <SettleFx win={game.result === "win"} />}
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
