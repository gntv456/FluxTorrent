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
          aria-hidden
          style={{ "--i": i } as React.CSSProperties}
        />
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
  const meSeat = game.seats.find((s) => s.seat === 0);
  // 我方代表全身立绘：身份口径与座位头像一致（地主 / AI / 农民）。
  const portraitSrc = meSeat?.role === "landlord"
    ? "/games/ddz/ddz-full-landlord.webp"
    : meSeat?.is_ai
      ? "/games/ddz/ddz-full-ai.webp"
      : "/games/ddz/ddz-full-farmer.webp";
  const detail: { text: string; icon?: string }[] = [
    {
      text: t.detailStake.replace("{n}", String(game.stake)),
      icon: "/games/ddz/ddz-icon-stake.webp",
    },
    {
      text: t.detailMult.replace("{n}", String(game.multiplier)),
      icon: "/games/ddz/ddz-icon-mult.webp",
    },
  ];
  if (game.bomb_count > 0)
    detail.push({
      text: t.detailBomb.replace("{n}", String(game.bomb_count)),
      icon: "/games/ddz/ddz-icon-bomb.webp",
    });
  if (meSeat?.role === "landlord") detail.push({ text: t.detailShareLandlord });
  else if (meSeat?.role === "farmer") detail.push({ text: t.detailShareFarmer });
  const tone =
    game.result === "win"
      ? "is-win"
      : game.result === "lose"
        ? "is-lose"
        : "is-draw";
  return (
    <div className={`dz-settle ${tone}`} role="status">
      {game.result !== null && <SettleFx win={game.result === "win"} />}
      {game.result === "win" && (
        <div className="dz-settle-badge" aria-hidden>
          <img
            src="/games/ddz/ddz-crown.webp"
            alt=""
            draggable="false"
          />
        </div>
      )}
      {game.result !== null && meSeat && (
        <div className="dz-settle-portrait" aria-hidden>
          <img src={portraitSrc} alt="" draggable="false" />
        </div>
      )}
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
      {game.spring && (
        <p className="dz-spring">
          <img className="dz-ico" src="/games/ddz/ddz-icon-spring.webp" alt="" draggable="false" />
          {t.spring}
        </p>
      )}
      {game.result !== null && detail.length > 0 && (
        <div className="dz-settle-detail" aria-label={t.detailTitle}>
          <span className="dz-settle-detail-title">{t.detailTitle}</span>
          {detail.map((d, i) => (
            <span key={i} className="dz-detail-chip">
              {d.icon && (
                <img className="dz-ico" src={d.icon} alt="" draggable="false" />
              )}
              {d.text}
            </span>
          ))}
        </div>
      )}
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
