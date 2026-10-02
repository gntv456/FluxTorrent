"use client";

import { PANEL_LG } from "@/lib/ui-classes";

import { fmt, fmtCur } from "@/i18n/config";
import { eggText, type EggPrize } from "@/lib/games";
import {
  FarmPlot,
  MarketCard,
  type Crop,
  type Plot,
} from "@/components/game/farm-field";
import { FarmLandStrip, type ActFn as Act } from "@/components/game/farm-land";
import type { FarmData } from "./_inner";

/** 农场专注页分区件（从 games/farm/_inner.tsx 按域拆出）：
 *  我的田（一键收获 + 逐块浇水/收获 + 土地阶梯）与图鉴式行情两区，
 *  数据装载与动作编排留在 _inner.tsx。 */

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
  const planted = plots.filter((p) => p).length;
  return (
    <section className={PANEL_LG}>
      <div className="sw-farm-strip">
        <span>
          {tf.fsPlanted
            .replace("{a}", String(planted))
            .replace("{b}", String(plots.length))}
        </span>
        <span className="sw-farm-dot">·</span>
        <span>{tf.fsReady.replace("{n}", String(readyPlots.length))}</span>
      </div>
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
      {/* 样图⑤：4 列紧凑方格田（大屏 6 列），地块多时也不撑长页 */}
      <div className="grid grid-cols-4 gap-2 sm:gap-3 md:grid-cols-6">
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
                    prize?: EggPrize;
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
                      ) + eggText(tf, r.prize);
                },
                (d) => {
                  const r = d as unknown as {
                    withered?: boolean;
                    doubled?: boolean;
                    prize?: EggPrize;
                  };
                  if (r.withered) return "lose";
                  return r.doubled || r.prize?.kind === "item"
                    ? "jackpot"
                    : "win";
                },
              )
            }
            t={tf}
          />
        ))}
      </div>

      {data.land && (
        <FarmLandStrip
          land={data.land}
          tf={tf}
          currency={currency}
          busy={busy}
          act={act}
        />
      )}
    </section>
  );
}

/** 图鉴式行情：作物卡网格（点卡种植） */
export function MarketSection({
  crops,
  onPlant,
  busy,
  tf,
}: {
  crops: Crop[];
  onPlant: (cropId: number) => void;
  /** 播种请求进行中（防连点，与田块按钮同纪律） */
  busy: boolean;
  tf: Record<string, string>;
}) {
  return (
    <section className={PANEL_LG}>
      <h2 className="mb-3 font-display text-base">{tf.market}</h2>
      <div className="grid grid-cols-2 gap-3 md:grid-cols-3 lg:grid-cols-5">
        {crops.map((c) => (
          <MarketCard
            key={c.id}
            crop={c}
            onPlant={onPlant}
            busy={busy}
            t={tf}
          />
        ))}
      </div>
    </section>
  );
}
