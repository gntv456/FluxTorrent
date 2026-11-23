"use client";

import { ResultFlash } from "@/components/game/game-kit-feedback";
import { ScratchCoat, type CoatOutcome } from "@/components/game/scratch-coat";
import { SwCtaRow, SwStatRow } from "@/components/game/sw-panels";

export type ScratchPhase = "idle" | "buying" | "scratchable" | "done";

export interface ScratchPlayCardProps {
  bet: number;
  onBet: (v: number) => void;
  maxBet: number;
  phase: ScratchPhase;
  outcome: CoatOutcome | null;
  hitIndex: number | null;
  revealed: number[];
  onReveal: (i: number) => void;
  onRevealAll: () => void;
  headTicket: string;
  tierMaxText: string;
  coatProgress: string;
  decoyLabel: string;
  autoScratchLabel: string;
  stats: {
    playsLb: string;
    playsVal: string;
    maxLb: string;
    maxVal: string;
    leftLb: string;
    leftVal: string;
  };
  cta: {
    primaryLabel: string;
    goldLabel: string;
    onPrimary: () => void;
    onGold: () => void;
    primaryDisabled: boolean;
    goldDisabled: boolean;
    goldBusy: boolean;
    autoLabel: string;
    autoActive: boolean;
    autoDisabled: boolean;
    onAuto: () => void;
  };
  flash: { kind: "win" | "lose" | "tie" | "jackpot"; text: string } | null;
  bannerTitle: string;
  tagline: string;
  currency: string;
}

/**
 * 刮刮乐主玩法票卡（样图红黄促销风落地）：红 Banner + 黄票卡 + 票档 +
 * 涂层格 + 大奖预览 + 统计 + CTA。纯呈现哑件，状态机与账务留在 _inner。
 */
export function ScratchPlayCard(props: ScratchPlayCardProps) {
  const { cta, stats } = props;
  const tiers = [10, 100, 1000];
  return (
    <div className="sp-stage-col">
      <div className="sp-ticket">
        <div className="sp-banner">
          <span className="sp-deco d1" aria-hidden>📣</span>
          <span className="sp-deco d2" aria-hidden>✨</span>
          <span className="sp-deco d3" aria-hidden>🎁</span>
          <span className="sp-deco d4" aria-hidden>🎀</span>
          <div className="sp-banner-title">
            <span className="sp-row2">{props.bannerTitle}</span>
          </div>
          <div className="sp-ribbon">{props.tagline}</div>
        </div>

        <div className="sp-body">
          <div className="sw-tiers">
            {tiers.map((v) => (
              <button
                key={v}
                type="button"
                onClick={() => props.onBet(v)}
                disabled={
                  props.phase === "buying" ||
                  props.phase === "scratchable" ||
                  v > props.maxBet ||
                  cta.autoActive
                }
                aria-pressed={props.bet === v}
                className={`sw-tier${
                  props.bet === v ? (v === 1000 ? " on-gold" : " on-mint") : ""
                }`}
              >
                <div className="sw-tier-b num">
                  {v} {props.currency}
                </div>
                <div className="sw-tier-s">{props.tierMaxText}</div>
              </button>
            ))}
          </div>

          <div className="sp-frame">
            <ScratchCoat
              outcome={props.phase === "idle" ? null : props.outcome}
              hitIndex={props.hitIndex}
              revealed={props.revealed}
              onReveal={props.onReveal}
              decoyLabel={props.decoyLabel}
              disabled={props.phase !== "scratchable"}
            />
          </div>

          <div className="sw-env-info">
            <span>{props.headTicket}</span>
            <span className="num">{props.coatProgress}</span>
          </div>
          <div className="sp-coat-actions">
            {props.phase === "scratchable" && (
              <button
                type="button"
                className="sw-reveal-all"
                onClick={props.onRevealAll}
              >
                {props.autoScratchLabel}
              </button>
            )}
          </div>

          <ResultFlash
            kind={props.flash?.kind ?? null}
            text={props.flash?.text ?? null}
          />
        </div>
      </div>

      <SwStatRow
        items={[
          { lb: stats.playsLb, vl: stats.playsVal },
          { lb: stats.maxLb, vl: stats.maxVal, tone: "gold" },
          { lb: stats.leftLb, vl: stats.leftVal, tone: "green" },
        ]}
      />

      <SwCtaRow
        primaryLabel={cta.primaryLabel}
        goldLabel={cta.goldLabel}
        onPrimary={cta.onPrimary}
        onGold={cta.onGold}
        primaryDisabled={cta.primaryDisabled}
        goldDisabled={cta.goldDisabled}
        goldBusy={cta.goldBusy}
        autoLabel={cta.autoLabel}
        autoActive={cta.autoActive}
        autoDisabled={cta.autoDisabled}
        onAuto={cta.onAuto}
      />
    </div>
  );
}
