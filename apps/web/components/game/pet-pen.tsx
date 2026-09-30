"use client";

export interface PetStatus {
  species: string;
  name: string;
  level: number;
  exp: number;
  exp_to_next: number;
  hunger: number;
  energy: number;
  pending: number;
  yield_permille: number;
  feed_cost: number;
  digest_per_hour: number;
  max_level: number;
}

/** 成长线：蛋 → 雏 → 成鸟 → 猛禽 → 龙。等级越高越像样（养成看点）。 */
const GROW = ["🥚", "🐣", "🐥", "🐤", "🐔", "🦆", "🦢", "🦅", "🦉", "🐲"];

/** 宠物栏：成长形象 + 状态条。纯展示，动作在页面。 */
export function PetPen({
  status,
  hungerLabel,
  expLabel,
}: {
  status: PetStatus;
  hungerLabel: string;
  expLabel: string;
}) {
  const i = Math.min(Math.max(status.level - 1, 0), GROW.length - 1);
  const maxed = status.level >= status.max_level;
  return (
    <div className="pp-pen">
      <div className="pp-stage">
        <span className={`pp-pet${maxed ? " max" : ""}`} aria-hidden>
          {GROW[i]}
        </span>
        <span className="pp-ring" aria-hidden />
      </div>
      <div className="pp-bars">
        <div className="pp-bar">
          <span className="pp-bar-lb">{hungerLabel}</span>
          <span className="pp-track">
            <i
              className="pp-fill hunger"
              style={{ width: `${Math.max(0, Math.min(100, status.hunger))}%` }}
            />
          </span>
          <span className="num pp-bar-n">{status.hunger}%</span>
        </div>
        <div className="pp-bar">
          <span className="pp-bar-lb">{expLabel}</span>
          <span className="pp-track">
            <i
              className="pp-fill exp"
              style={{ width: `${maxed ? 100 : status.exp % 100}%` }}
            />
          </span>
          <span className="num pp-bar-n">
            {maxed ? "MAX" : `${status.exp % 100}/100`}
          </span>
        </div>
      </div>
    </div>
  );
}
