"use client";

/**
 * 星尘信封 · 3×2 涂层格（样图②）：一局 = 一张信封，六个涂层格，
 * 其中一格藏着本张结果（服务端在买卡时已定死），其余是谢谢参与。
 * 纯呈现层 —— 刮开的顺序由玩家选，结果不因顺序改变。
 */

export interface CoatOutcome {
  mult: number;
  icon?: string;
  rarity: number;
  /** 本张净收（flash 文案用，格子不显示） */
  net: number;
}

export function ScratchCoat({
  outcome,
  hitIndex,
  revealed,
  onReveal,
  decoyLabel,
  disabled,
}: {
  outcome: CoatOutcome | null;
  /** 奖品格下标（buy 时随机定，与结果无关纯粹是揭开仪式） */
  hitIndex: number | null;
  revealed: number[];
  onReveal: (i: number) => void;
  decoyLabel: string;
  disabled?: boolean;
}) {
  const cells = [0, 1, 2, 3, 4, 5];
  return (
    <div className="sw-coat-grid">
      {cells.map((i) => {
        const open = revealed.includes(i);
        const isHit = open && i === hitIndex && outcome !== null;
        return (
          <button
            key={i}
            type="button"
            className={`sw-coat-cell${open ? " open" : ""}${
              isHit ? " sw-coat-hit" : ""
            }`}
            disabled={disabled || open || outcome === null}
            onClick={() => onReveal(i)}
            aria-label={open ? undefined : "scratch"}
          >
            <span className="sw-coat-face">
              {isHit ? (
                <>
                  <span className="ic" aria-hidden>
                    {outcome?.icon || "✨"}
                  </span>
                  <span className="mx num">
                    {outcome?.mult.toFixed(1)}x
                  </span>
                </>
              ) : open ? (
                <span className="decoy">{decoyLabel}</span>
              ) : (
                <span aria-hidden>?</span>
              )}
            </span>
            {!open && <span className="sw-coat-lid" aria-hidden>刮</span>}
          </button>
        );
      })}
    </div>
  );
}
