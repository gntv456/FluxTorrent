"use client";

/**
 * 农场单块田组件（CropArt / MarketCard / Crop 已拆至
 * components/game/farm-art.tsx，295 行门禁）。本文件保留 FarmPlot + Plot。
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
  ready: boolean;
  /** 超过有效期（farm_wither_days）未收 → 枯萎：收获作废，只能清理重种 */
  withered?: boolean;
}

// 拆出到 farm-art.tsx 的符号在此转发导出，保持既有 import 路径不变
export { CropArt, MarketCard } from "./farm-art";
export type { Crop } from "./farm-art";

const PLOT_EMPTY_CLS =
  "flex min-h-[132px] flex-col items-center justify-between " +
  "rounded-[var(--r-md)] border border-dashed p-3 text-center " +
  "active:scale-[0.98] disabled:opacity-60";

const WATER_BTN_CLS =
  "min-h-[34px] flex-1 rounded-full bg-sky-soft px-2 text-[11px] " +
  "font-bold text-ink active:scale-[0.97] disabled:opacity-50";

const HARVEST_BTN_CLS =
  "min-h-[34px] flex-1 rounded-full px-2 text-[11px] font-bold " +
  "active:scale-[0.97] disabled:opacity-40";

const HARVEST_WITHERED_CLS =
  "border border-[var(--border-deep)] bg-[var(--surface-card)] text-sub";

const PLOT_CLS =
  "flex min-h-[132px] flex-col justify-between rounded-[var(--r-md)] " +
  "border bg-[var(--surface-card)] p-3 shadow-[var(--shadow-card)]";

/** 单块田：进度环 + 阶段造型 + 浇水/收获/清理动作 */
export function FarmPlot({
  slot,
  plot,
  crop,
  now,
  busy,
  picking,
  onPlant,
  onWater,
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
        className={`${PLOT_EMPTY_CLS} ${
          picking
            ? "border-sun bg-sun-soft ring-2 ring-sun"
            : "border-[var(--border-deep)] bg-[var(--surface-raised)]"
        }`}
      >
        <CropArt cropId={slot} stage={0} />
        <span className="text-xs font-bold">
          {t.plotEmpty.replace("{n}", String(slot))}
        </span>
        <span className="text-[11px] text-sub">{t.plotHint}</span>
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
        ? t.mature
        : `${Math.floor(left / 3600000)}h${Math.floor(
            (left % 3600000) / 60000,
          )}m`;

  const r = 14;
  const c = 2 * Math.PI * r;
  return (
    <div
      className={`${PLOT_CLS} ${
        withered
          ? "border-[var(--border-soft)] opacity-80"
          : ready
            ? "border-sun"
            : "border-line"
      }`}
    >
      <div className="flex items-start justify-between gap-2">
        <div className="min-w-0">
          <p className="truncate text-xs font-bold">
            <span className={withered ? "text-sub line-through" : undefined}>
              {crop?.name ?? plot.crop_name}
            </span>
            {withered ? (
              <span className="ml-1 text-[11px] font-black text-danger">
                🥀 {t.witheredTag}
              </span>
            ) : (
              ready && (
                <span className="ml-1 text-[11px] text-mint">
                  ✓ {t.matureShort}
                </span>
              )
            )}
          </p>
          <p className="text-[11px] text-sub">
            {withered
              ? t.witheredHint
              : `${ready ? t.ready : t.readyIn.replace("{t}", leftText)}${
                  plot.watered ? ` · ${t.watered}` : ""
                }`}
          </p>
        </div>
        {!withered && (
          <svg viewBox="0 0 36 36" className="h-9 w-9 shrink-0" aria-hidden>
            <circle
              cx="18"
              cy="18"
              r={r}
              fill="none"
              stroke="var(--surface-sunken)"
              strokeWidth="4"
            />
            <circle
              cx="18"
              cy="18"
              r={r}
              fill="none"
              stroke={ready ? "var(--sun)" : "var(--mint)"}
              strokeWidth="4"
              strokeLinecap="round"
              strokeDasharray={c}
              strokeDashoffset={c * (1 - progress)}
              transform="rotate(-90 18 18)"
            />
          </svg>
        )}
      </div>
      <div className={withered ? "opacity-60 grayscale" : undefined}>
        <CropArt cropId={plot.crop_id} stage={stage} />
      </div>
      <div className="mt-1 flex gap-1.5">
        {!withered && !plot.watered && !ready && (
          <button
            type="button"
            onClick={() => onWater(slot)}
            disabled={busy}
            className={WATER_BTN_CLS}
          >
            {t.water}
          </button>
        )}
        <button
          type="button"
          onClick={() => onHarvest(slot)}
          disabled={busy || (!ready && !withered)}
          className={`${HARVEST_BTN_CLS} ${
            withered
              ? HARVEST_WITHERED_CLS
              : ready
                ? "bg-mint text-white"
                : "bg-[var(--surface-sunken)] text-sub"
          }`}
        >
          {withered ? t.cleanup : ready ? t.harvest : t.notReady}
        </button>
      </div>
    </div>
  );
}
