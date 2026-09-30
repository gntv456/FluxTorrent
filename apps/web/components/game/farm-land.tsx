"use client";

import { fmt, fmtCur } from "@/i18n/config";
import { BTN_SM_BOLD, BTN_SM_GHOST } from "@/lib/ui-classes";

/** 娱乐玩法的动作编排（`games/farm/_inner.tsx` 里的 `act`）：
 *  发请求 → 成功文案 → 顶栏浮层反馈 → 重拉数据，四处动作共用这一条。 */
export type ActFn = (
  path: string,
  body: unknown,
  ok: (d: never) => string,
  kindOf?: (d: never) => "win" | "lose" | "jackpot",
) => Promise<void>;

/** 一块地的持有状态：等级、周转、下一级报价（全部由服务端算，前端不抄公式） */
export interface LandPlot {
  slot: number;
  level: number;
  /** 买来的地（false = 免费送的那几块） */
  bought: boolean;
  /** 周转系数千分比：1000 = 原速，越小越快的反面（500 = 只花一半时间） */
  speed_permille: number;
  maxed: boolean;
  next_upgrade_price: number;
}

export interface FarmLand {
  free: number;
  cap: number;
  purchased: number;
  owned: number;
  /** 下一块可买的地块号；买满时服务端给 null（前端不自己推下一个数） */
  next_slot: number | null;
  next_land_price: number | null;
  max_level: number;
  plots: LandPlot[];
}

/** 周转加成说成「快百分之几」：1000‰ → 0%，700‰ → 快 30%。
 *  服务端只给千分比系数，这里只做单位换算，不再抄第二份步长口径。 */
function fastPct(speedPermille: number): number {
  return Math.round((1000 - speedPermille) / 10);
}

/** 行宽门禁（新文件 ≤80）把长 className 收在这里，JSX 只留引用 */
const HEAD_CLS =
  "mt-3 flex flex-wrap items-center justify-between gap-2 border-t " +
  "border-line pt-3";
const CHIP_CLS =
  "flex items-center gap-2 rounded-full border border-line bg-cloud " +
  "px-3 py-1 text-[11px]";
const IDLE_CLS =
  "flex items-center rounded-full border border-dashed border-line " +
  "px-3 py-1 text-[11px] text-sub";
const BUY_CLS = `${BTN_SM_BOLD} min-h-[30px] rounded-full px-3 text-[11px]`;

/** 土地阶梯条：我的地块（等级/周转/升级报价）+ 按阶梯买下一块。
 *  挂在「我的田地」面板底部，自己不再套一层 panel。
 *
 *  这一条只念服务端给的数：报价、档数、上限、满级判定都在
 *  `games::farm_land` 与 `games_http::farm_land`，前端不重算 ——
 *  站长改一次比率，这里跟着走，不需要第二份同步。 */
export function FarmLandStrip({
  land,
  tf,
  currency,
  busy,
  act,
}: {
  land: FarmLand;
  tf: Record<string, string>;
  currency: string;
  busy: boolean;
  act: ActFn;
}) {
  function buy() {
    if (land.next_slot === null || land.next_land_price === null) return;
    void act("/farm/land/buy", { slot: land.next_slot }, (d) => {
      const r = d as unknown as { slot: number; cost: number };
      return fmtCur(tf.landBuyOk, { slot: r.slot, cost: r.cost }, currency);
    });
  }

  function upgrade(p: LandPlot) {
    void act("/farm/land/upgrade", { slot: p.slot }, (d) => {
      const r = d as unknown as {
        slot: number;
        level: number;
        cost: number;
        speed_permille: number;
      };
      return fmtCur(
        tf.landUpOk,
        {
          slot: r.slot,
          n: r.level,
          cost: r.cost,
          p: fastPct(r.speed_permille),
        },
        currency,
      );
    });
  }

  return (
    <>
      <div className={HEAD_CLS}>
        <h3 className="font-display text-sm">{tf.landTitle}</h3>
        <span className="text-xs text-sub">
          {fmt(tf.landHint, {
            n: land.owned,
            cap: land.cap,
            free: land.free,
            bought: land.purchased,
          })}
        </span>
      </div>

      <div className="mt-2 flex flex-wrap gap-2">
        {land.plots.map((p) => (
          <span
            key={p.slot}
            className={CHIP_CLS}
          >
            <b className="font-display">{fmt(tf.landSlot, { n: p.slot })}</b>
            <span>{fmt(tf.landLv, { n: p.level })}</span>
            {p.level > 1 && (
              <span className="text-sub">
                {fmt(tf.landFast, { p: fastPct(p.speed_permille) })}
              </span>
            )}
            {p.bought && (
              <span className="rounded-full bg-sun-soft px-2 text-[10px]">
                {tf.landBoughtTag}
              </span>
            )}
            {p.maxed ? (
              <span className="text-[10px] text-sub">{tf.landMax}</span>
            ) : (
              <button
                type="button"
                onClick={() => upgrade(p)}
                disabled={busy}
                className={`${BTN_SM_GHOST} min-h-[26px] px-2 text-[10px]`}
              >
                {fmtCur(tf.landUp, { cost: p.next_upgrade_price }, currency)}
              </button>
            )}
          </span>
        ))}

        {land.next_slot === null ? (
          <span className={IDLE_CLS}>
            {fmt(tf.landFull, { cap: land.cap })}
          </span>
        ) : (
          <button
            type="button"
            onClick={buy}
            disabled={busy}
            className={BUY_CLS}
          >
            {fmtCur(
              tf.landBuy,
              {
                n: land.next_slot,
                cost: land.next_land_price ?? 0,
              },
              currency,
            )}
          </button>
        )}
      </div>

      <p className="mt-2 text-[11px] leading-relaxed text-sub">
        {tf.landNote}
      </p>
    </>
  );
}
