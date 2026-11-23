"use client";
import { useI18n } from "@/i18n/client";
import { fmtCur } from "@/i18n/config";
import { fmtMult } from "@/lib/games";
import { ChipDenoms } from "./chip-denoms";
import type { RoomState } from "./room-types";

/** 房间控制区：筹码面额栏、大/小下注盘（豹子仅展示位）、我的注汇总。 */
export function RoomControls({
  bet,
  onBet,
  maxBet,
  busy,
  closed,
  room,
  winMult,
  myTotal,
  onPlace,
}: {
  bet: number;
  onBet: (n: number) => void;
  maxBet: number;
  busy: boolean;
  closed: boolean;
  room: RoomState | null;
  winMult: number;
  myTotal: number;
  onPlace: (side: "small" | "big") => void;
}) {
  const { dict, currency } = useI18n();
  const tg = dict.games.bigsmall;

  return (
    <div className="flex flex-col gap-3">
      <ChipDenoms
        value={bet}
        onChange={onBet}
        maxBet={maxBet}
        disabled={busy || closed}
      />
      <div className="bs-picks">
        <button
          type="button"
          onClick={() => onPlace("small")}
          disabled={busy || closed}
          className="bs-pick"
        >
          <div className="bs-pick-t">{tg.bsPickSmall}</div>
          <div className="bs-pick-s">
            {tg.bsPickSmallSub.replace("{n}", fmtMult(winMult))}
          </div>
          <div className="bs-pick-s num">
            {fmtCur("{n}", { n: room?.round.total_small ?? 0 }, currency)}
          </div>
        </button>
        <button
          type="button"
          onClick={() => onPlace("big")}
          disabled={busy || closed}
          className="bs-pick gold"
        >
          <div className="bs-pick-t">{tg.bsPickBig}</div>
          <div className="bs-pick-s">
            {tg.bsPickBigSub.replace("{n}", fmtMult(winMult))}
          </div>
          <div className="bs-pick-s num">
            {fmtCur("{n}", { n: room?.round.total_big ?? 0 }, currency)}
          </div>
        </button>
        <div className="bs-pick" aria-disabled="true">
          <div className="bs-pick-t">{tg.bsPickTriple}</div>
          <div className="bs-pick-s">{tg.bsPickTripleSub}</div>
        </div>
      </div>
      <p className="num text-center text-xs text-sub">
        {fmtCur(tg.roomMine, { n: myTotal }, currency)}
      </p>
    </div>
  );
}
