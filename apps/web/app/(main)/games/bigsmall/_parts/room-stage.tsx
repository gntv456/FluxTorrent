"use client";
import { useI18n } from "@/i18n/client";
import { fmtCur } from "@/i18n/config";
import { fmtMult } from "@/lib/games";
import { DieFace } from "@/components/game/die-face";
import { StreakPill } from "@/components/game/streak-pill";
import {
  BTN_RETRY_PILL,
  PANEL_WARN_ORANGE,
  STAGE_STAT_LINE,
} from "@/lib/ui-classes";
import { DealerArt } from "./dealer-art";
import type { RoomState } from "./room-types";

/** 房间舞台：荷官立绘 + 上期面板 + 倒计时框 + 骰子台，下方接统计细条。
 *  （在线头像圈 PlayerRail 暂未接入，见下方注释。） */
export function RoomStage({
  room,
  winMult,
  streak,
  todayPlays,
  limitLeft,
  err,
  unknown,
  onRetry,
}: {
  room: RoomState | null;
  winMult: number;
  streak: number;
  todayPlays: number;
  limitLeft: number | null;
  err: string | null;
  unknown: boolean;
  onRetry: () => void;
}) {
  const { dict, currency } = useI18n();
  const tg = dict.games.bigsmall;
  const remaining = room?.round.remaining ?? 0;
  const closed = remaining <= 0;
  const dice = room?.last?.dice ?? [1, 1, 1];
  const last = room?.last ?? null;

  const regionLabel = (region: string) =>
    region === "triple"
      ? tg.bsPickTriple
      : region === "big"
        ? tg.bsPickBig
        : tg.bsPickSmall;

  return (
    <div className="flex w-full flex-col items-center gap-3">
      <div className="bs-stage">
        <DealerArt />
        <div className="bs-stage-top">
          <span className="bs-badge-round">
            {tg.roomRound.replace("{n}", String(room?.round.id ?? "—"))}
          </span>
        </div>

        {last && (
          <div className="bs-last-panel">
            <div className="bs-last-row1">{tg.roomLast}</div>
            <div className="bs-last-row2">
              <span className="bs-last-dice">
                {last.dice.map((d, i) => (
                  <DieFace key={i} n={d} size={26} />
                ))}
              </span>
              <span className="bs-last-result">
                {regionLabel(last.region)} · {last.sum}
              </span>
            </div>
          </div>
        )}

        <div className="bs-count-box">
          <div className="bs-count-label">{tg.roomCountdownLabel}</div>
          <div className="bs-count-num">
            {closed ? (
              <span>{tg.roomClosed}</span>
            ) : (
              <>
                {remaining}
                <em>s</em>
              </>
            )}
          </div>
        </div>

        <div className="bs-dice-tray">
          <p className="bs-dice-cap">
            {last
              ? fmtCur(tg.srResult, { n: last.sum }, currency)
              : tg.bsDiceWait}
          </p>
          <div className="bs-dice" aria-hidden>
            {dice.map((d, i) => (
              <DieFace key={i} n={d} />
            ))}
          </div>
        </div>
      </div>

      {/* 布局优化（2026-11-23）：连胜 + 三格统计压成一行细条。原「在线
          玩家」头像圈（PlayerRail）已移除——它是 7 个写死的演示假名，
          房间口并无玩家列表字段，占一整行 86px 且会让人误以为是真人。
          组件文件保留：接口日后给出真实玩家列表时按原 DOM 结构接回。 */}
      <div className={STAGE_STAT_LINE}>
        <StreakPill n={streak} label={tg.streak} />
        <span>
          {tg.bsStatPlays} <b className="num text-ink">{todayPlays}</b>
        </span>
        <span>
          {tg.bsStatMax}{" "}
          <b className="num text-[var(--warning)]">{fmtMult(winMult)}x</b>
        </span>
        <span>
          {tg.bsStatLeft}{" "}
          <b className="num text-[var(--mint)]">
            {limitLeft != null ? String(limitLeft) : "—"}
          </b>
        </span>
      </div>
      {err && <p className="text-xs text-[var(--danger-text)]">{err}</p>}
      {unknown && (
        <div className={PANEL_WARN_ORANGE}>
          <p>{dict.review.resultUnknownBody}</p>
          <button
            type="button"
            className={BTN_RETRY_PILL}
            onClick={onRetry}
          >
            {dict.review.retrySameKey}
          </button>
        </div>
      )}
    </div>
  );
}
