"use client";

import { moleIcon, moleTag } from "@/lib/mole-tier";
import type { MoleSpawn } from "@/components/game/use-mole-play";

/** 3×3 洞阵：每只冒头地鼠是一个大热区按钮，空挥不罚、只演一下尘。 */
const SHOWCASE: Record<number, string> = {
  0: "🐹",
  4: "🐷",
  7: "🐰",
};

export function MoleBoard({
  active,
  idle,
  ticket,
  left,
  roundMs,
  score,
  combo,
  miss,
  playing,
  reduced,
  labels,
  onWhack,
}: {
  active: Record<number, MoleSpawn>;
  idle: boolean;
  ticket: number;
  left: number;
  roundMs: number;
  score: number;
  combo: number;
  miss: number;
  playing: boolean;
  reduced: boolean;
  labels: {
    hole: string;
    bomb: string;
    left: string;
    score: string;
    combo: string;
    idle: string;
  };
  onWhack: (hole: number) => void;
}) {
  const pct = roundMs > 0 ? (left / roundMs) * 100 : 0;
  const sec = Math.ceil(left / 1000);
  return (
    <div className={`mo-board${reduced ? " flat" : ""}${idle ? " idle" : ""}`}>
      <div className="mo-hud">
        <div className="mo-clock">
          <span className="mo-clock-ic" aria-hidden>
            ⏱
          </span>
          <span className="mo-clock-lb">{labels.left}</span>
          <b className="num">{sec}</b>
        </div>
        <div className="mo-track" aria-hidden>
          <span className="mo-track-fill" style={{ width: `${pct}%` }} />
        </div>
        <div className="mo-score-chip">
          <span className="mo-score-lb">{labels.score}</span>
          <b className="mo-score num">{score}</b>
        </div>
        <span className="mo-combo num">
          {labels.combo.replace("{n}", String(combo))}
        </span>
      </div>
      <div className="mo-grid">
        {Array.from({ length: 9 }, (_, hole) => {
          const sp = active[hole];
          const up = sp !== undefined;
          const bomb = up && sp.idx < 0;
          const face = up
            ? bomb
              ? "💣"
              : moleIcon(sp.value, ticket)
            : idle
              ? SHOWCASE[hole] ?? ""
              : "";
          return (
            <button
              key={hole}
              type="button"
              className={`mo-hole${up ? " up" : ""}${bomb ? " bomb" : ""}`}
              aria-label={`${labels.hole} ${hole + 1}${
                up ? ` ${bomb ? labels.bomb : sp.label}` : ""
              }`}
              onClick={() => onWhack(hole)}
            >
              <span className="mo-rim" aria-hidden />
              <span className="mo-dirt" aria-hidden />
              <span className="mo-mole" aria-hidden>
                <span className="mo-face">
                  {face}
                </span>
                {up && (
                  <span className={`mo-tag num${bomb ? " neg" : ""}`}>
                    {bomb
                      ? labels.bomb
                      : moleTag(sp.value)}
                  </span>
                )}
              </span>
              <span className="mo-hole-num num" aria-hidden>
                {hole + 1}
              </span>
            </button>
          );
        })}
      </div>
      <span key={miss} className="mo-dust" aria-hidden />
      {!playing && (
        <div className="mo-idle" aria-hidden>
          {labels.idle}
        </div>
      )}
    </div>
  );
}
