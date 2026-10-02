"use client";

/**
 * 农场单块田组件（CropArt / MarketCard / Crop 已拆至
 * components/game/farm-art.tsx，295 行门禁）。本文件保留 FarmPlot + Plot。
 *
 * 2026-10 样图⑤对齐：田块从大卡改成紧凑方格（4 列 aspect-square），
 * 作物色淡底 + 右下状态角牌；浇水收进角落小钮（显式动作，避免点 tile
 * 误触扣费）；成熟/枯萎整块可点（收获/清理）。
 */

import { CropArt } from "./farm-art";
import type { Crop } from "./farm-art";

export interface Plot {
  slot: number;
  crop_id: number;
  crop_name: string;
  planted_at: string;
  ready_at: string;
  watered: boolean;
  /** 施肥（每茬一次，催熟 30 分钟） */
  fertilized?: boolean;
  ready: boolean;
  /** 超过有效期（farm_wither_days）未收 → 枯萎：收获作废，只能清理重种 */
  withered?: boolean;
}

// 拆出到 farm-art.tsx 的符号在此转发导出，保持既有 import 路径不变
export { CropArt, MarketCard } from "./farm-art";
export type { Crop } from "./farm-art";

/** 作物淡底色：与其余农场件同一组色 token（色相环错开） */
const CROP_TINTS = [
  "--sun",
  "--mint",
  "--candy",
  "--coral",
  "--sky",
  "--indigo",
];

const BADGE_CLS =
  "num rounded-full bg-[var(--surface-card)]/90 px-1.5 py-0.5 " +
  "text-[9px] font-black leading-none shadow-[0_1px_3px_rgba(44,62,92,0.25)]";

const WATER_CHIP_CLS =
  "absolute bottom-1 left-1 grid h-6 w-6 place-items-center rounded-full " +
  "bg-sky-soft text-[12px] shadow-[0_1px_4px_rgba(44,62,92,0.3)] " +
  "active:scale-95 disabled:opacity-50";

/** 单块田：紧凑方格（样图⑤）。空地=虚线+号；种植中=作物色底+倒计时角牌
 *  + 浇水/施肥角钮；成熟/枯萎=整块可点（收获/清理）。 */
export function FarmPlot({
  slot,
  plot,
  crop,
  now,
  busy,
  picking,
  onPlant,
  onWater,
  onFertilize,
  onHarvest,
  t,
}: {
  slot: number;
  plot: Plot | undefined;
  crop: Crop | undefined;
  now: number | null;
  busy: boolean;
  /** 正被选为播种目标（高亮，避免用户点完不知道点的是哪块） */
  picking?: boolean;
  onPlant: (slot: number) => void;
  onWater: (slot: number) => void;
  /** 施肥（每茬一次，催熟 30 分钟） */
  onFertilize?: (slot: number) => void;
  onHarvest: (slot: number) => void;
  t: Record<string, string>;
}) {
  if (!plot) {
    return (
      <button
        type="button"
        onClick={() => onPlant(slot)}
        disabled={busy}
        aria-pressed={picking}
        className={`fp-plot flex aspect-square flex-col items-center justify-center gap-0.5 rounded-[14px] border border-dashed p-1 text-center active:scale-[0.98] disabled:opacity-60 ${
          picking
            ? "border-sun bg-sun-soft ring-2 ring-sun"
            : "border-[var(--border-deep)] bg-[var(--surface-raised)]"
        }`}
      >
        <span aria-hidden className="text-xl leading-none text-sub">
          +
        </span>
        <span className="text-[10px] font-bold leading-tight">
          {t.plotEmpty.replace("{n}", String(slot))}
        </span>
      </button>
    );
  }
  const readyAt = new Date(plot.ready_at).getTime();
  const plantedAt = new Date(plot.planted_at).getTime();
  const leftMs = now === null ? null : readyAt - now;
  const totalMs = Math.max(1, readyAt - plantedAt);
  const progress =
    leftMs === null ? 0 : Math.min(1, Math.max(0, 1 - leftMs / totalMs));
  const ready = plot.ready || (leftMs !== null && leftMs <= 0);
  const withered = !!plot.withered;
  const stage: 0 | 1 | 2 | 3 = ready
    ? 3
    : progress < 0.34
      ? 1
      : progress < 0.8
        ? 2
        : 2;
  const left = leftMs === null ? null : Math.max(0, leftMs);
  const leftText =
    left === null
      ? "…"
      : left <= 0
        ? t.matureShort
        : `${Math.floor(left / 3600000)}h${Math.floor(
            (left % 3600000) / 60000,
          )}m`;

  const tint = `color-mix(in srgb, var(${
    CROP_TINTS[(plot.crop_id - 1) % CROP_TINTS.length]
  }) 16%, white)`;

  // 成熟/枯萎：整块就是动作钮（收获/清理）；种植中：tile 不可点，浇水走角钮
  const actionable = ready || withered;
  const inner = (
    <>
      <div className="pointer-events-none flex w-full flex-1 items-center justify-center">
        <div className={withered ? "opacity-60 grayscale" : undefined}>
          <CropArt cropId={plot.crop_id} stage={stage} />
        </div>
      </div>
      <span
        className={`absolute bottom-1 right-1 ${BADGE_CLS} ${
          withered
            ? "text-danger"
            : ready
              ? "border border-sun text-mint"
              : "text-sub"
        }`}
      >
        {withered
          ? `🥀${t.witheredTag}`
          : ready
            ? `✓${t.matureShort}`
            : leftText}
      </span>
      {plot.watered && !ready && !withered && (
        <span
          aria-hidden
          className="absolute right-1 top-1 text-[10px] opacity-70"
        >
          💧
        </span>
      )}
      {plot.fertilized && !ready && !withered && (
        <span
          aria-hidden
          className="absolute right-6 top-1 text-[10px] opacity-70"
        >
          🌱
        </span>
      )}
      {!withered && !ready && (
        <div className="absolute bottom-1 left-1 flex gap-1">
          {!plot.watered && (
            <button
              type="button"
              onClick={(e) => {
                e.stopPropagation();
                onWater(slot);
              }}
              disabled={busy}
              title={t.water}
              aria-label={t.water}
              className={WATER_CHIP_CLS}
            >
              💧
            </button>
          )}
          {!plot.fertilized && onFertilize && (
            <button
              type="button"
              onClick={(e) => {
                e.stopPropagation();
                onFertilize(slot);
              }}
              disabled={busy}
              title={t.fert}
              aria-label={t.fert}
              className={WATER_CHIP_CLS}
            >
              🌱
            </button>
          )}
        </div>
      )}
    </>
  );
  if (!actionable) {
    return (
      <div
        className={`fp-plot relative flex aspect-square items-stretch rounded-[14px] border p-1 ${
          ready ? "border-sun" : "border-line"
        } bg-[var(--surface-card)] shadow-[var(--shadow-card)]`}
        style={{ background: tint }}
      >
        {inner}
      </div>
    );
  }
  return (
    <button
      type="button"
      onClick={() => onHarvest(slot)}
      disabled={busy}
      title={withered ? t.cleanup : t.harvest}
      className={`fp-plot relative flex aspect-square items-stretch rounded-[14px] border p-1 text-left active:scale-[0.98] disabled:opacity-60 ${
        withered
          ? "border-[var(--border-soft)] bg-[var(--surface-card)] opacity-80"
          : "border-sun bg-sun-soft"
      } shadow-[var(--shadow-card)]`}
      style={withered ? undefined : { background: tint }}
    >
      {inner}
    </button>
  );
}
