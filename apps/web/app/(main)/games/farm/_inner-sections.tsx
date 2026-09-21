"use client";

import { PANEL_LG } from "@/lib/ui-classes";

import { fmt, fmtCur } from "@/i18n/config";
import {
  FarmPlot,
  MarketCard,
  type Crop,
  type Plot,
} from "@/components/game/farm-field";
import type { FarmData } from "./_inner";

/** 农场专注页分区件（从 games/farm/_inner.tsx 按域拆出）：
 *  我的六块田（一键收获 + 逐块浇水/收获）与图鉴式行情两区，
 *  数据装载与动作编排留在 _inner.tsx。 */

type Act = (
  path: string,
  body: unknown,
  ok: (d: never) => string,
  kindOf?: (d: never) => "win" | "lose" | "jackpot",
) => Promise<void>;

/** 我的田地：地块网格 + 一键收获入口 */
export function MyFieldSection({
  data,
  plots,
  now,
  busy,
  picking,
  setPicking,
  readyPlots,
  tf,
  currency,
  onHarvestAll,
  act,
}: {
  data: FarmData;
  plots: (Plot | undefined)[];
  now: number | null;
  busy: boolean;
  picking: number | null;
  setPicking: (s: number) => void;
  readyPlots: (Plot | undefined)[];
  tf: Record<string, string>;
  currency: string;
  onHarvestAll: () => void;
  act: Act;
}) {
  return (
    <section className={PANEL_LG}>
      <div className="mb-3 flex flex-wrap items-center justify-between gap-2">
        <h2 className="font-display text-base">{tf.myField}</h2>
        <div className="flex flex-wrap items-center gap-2">
          {readyPlots.length > 1 && (
            <button
              type="button"
              onClick={onHarvestAll}
              disabled={busy}
              className="min-h-[34px] rounded-full bg-mint px-3 text-[11px] font-bold text-white active:scale-[0.97] disabled:opacity-50"
            >
              {tf.harvestAll.replace("{n}", String(readyPlots.length))}
            </button>
          )}
          <span className="text-xs text-sub">
            {picking ? fmt(tf.picking, { n: picking }) : tf.plotHint}
            {data.wither_days
              ? ` · ${fmt(tf.witherNote, { n: data.wither_days })}`
              : ""}
          </span>
        </div>
      </div>
      <div className="grid grid-cols-2 gap-3 md:grid-cols-3">
        {plots.map((p, i) => (
          <FarmPlot
            key={i}
            slot={i + 1}
            plot={p}
            crop={data.crops.find((c) => c.id === p?.crop_id)}
            now={now}
            busy={busy}
            picking={picking === i + 1}
            onPlant={(s) => setPicking(s)}
            onWater={(s) =>
              act("/farm/water", { slot: s }, (d) =>
                fmtCur(
                  tf.waterOk,
                  { n: (d as unknown as { cost?: number }).cost ?? 0 },
                  currency,
                ),
              )
            }
            onHarvest={(s) =>
              act(
                "/farm/harvest",
                { slot: s },
                (d) => {
                  const r = d as unknown as {
                    withered?: boolean;
                    crop: string;
                    amount: number;
                    doubled: boolean;
                  };
                  return r.withered
                    ? fmtCur(tf.witheredOk, { crop: r.crop }, currency)
                    : fmtCur(
                        tf.harvestOk,
                        {
                          crop: r.crop,
                          amount: r.amount,
                          doubled: r.doubled ? tf.doubled : "",
                        },
                        currency,
                      );
                },
                (d) => {
                  const r = d as unknown as {
                    withered?: boolean;
                    doubled?: boolean;
                  };
                  return r.withered ? "lose" : r.doubled ? "jackpot" : "win";
                },
              )
            }
            t={tf}
          />
        ))}
      </div>
    </section>
  );
}

/** 图鉴式行情：作物卡网格（点卡种植） */
export function MarketSection({
  crops,
  onPlant,
  tf,
}: {
  crops: Crop[];
  onPlant: (cropId: number) => void;
  tf: Record<string, string>;
}) {
  return (
    <section className={PANEL_LG}>
      <h2 className="mb-3 font-display text-base">{tf.market}</h2>
      <div className="grid grid-cols-2 gap-3 md:grid-cols-3 lg:grid-cols-5">
        {crops.map((c) => (
          <MarketCard key={c.id} crop={c} onPlant={onPlant} t={tf} />
        ))}
      </div>
    </section>
  );
}
