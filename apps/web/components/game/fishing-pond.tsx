"use client";

export type FishPhase =
  | "idle"
  | "casting"
  | "waiting"
  | "bite"
  | "reeling"
  | "done";

/**
 * 鱼塘舞台：水面 + 浮漂 + 涟漪 + 起竿窗口条。
 * 计时逻辑在页面（它持有定时器），这里只按 phase 画状态。
 */
export function FishingPond({
  phase,
  windowPct,
  waitingLabel,
  biteLabel,
}: {
  phase: FishPhase;
  /** 起竿窗口剩余比例（0..1），仅 bite 阶段有意义 */
  windowPct: number;
  waitingLabel: string;
  biteLabel: string;
}) {
  const inWater = phase === "waiting" || phase === "bite";
  return (
    <div className="fp-pond">
      <div className="fp-water" aria-hidden>
        <span className="fp-wave" />
        <span className="fp-wave w2" />
        {inWater && (
          <>
            <span className={`fp-bobber${phase === "bite" ? " bite" : ""}`} />
            <span className="fp-ripple" />
          </>
        )}
      </div>
      <div className="fp-hud">
        <span className="fp-label">
          {phase === "bite" ? biteLabel : waitingLabel}
        </span>
        {phase === "bite" && (
          <span className="fp-window" aria-hidden>
            <i style={{ width: `${Math.max(0, windowPct) * 100}%` }} />
          </span>
        )}
      </div>
    </div>
  );
}
