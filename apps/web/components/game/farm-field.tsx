"use client";

export interface Crop {
  id: number;
  name: string;
  seed_price: number;
  base_yield: number;
  grow_hours: number;
  market_price: number;
}

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

const CROP_COLORS = ["--sun", "--mint", "--candy", "--coral", "--sky", "--indigo"];

/** 作物造型：空地 / 出苗 / 生长 / 成熟 四阶段（成熟带摇曳） */
export function CropArt({ cropId, stage }: { cropId: number; stage: 0 | 1 | 2 | 3 }) {
  const color = `var(${CROP_COLORS[(cropId - 1) % CROP_COLORS.length]})`;
  if (stage === 0) {
    return (
      <svg viewBox="0 0 100 56" className="h-14 w-full" aria-hidden>
        <rect x="6" y="34" width="88" height="16" rx="4" fill="var(--surface-sunken)" />
        <path
          d="M10 40h80M10 46h80"
          stroke="var(--border-soft)"
          strokeWidth="2"
          strokeDasharray="6 6"
        />
      </svg>
    );
  }
  const h = stage === 1 ? 10 : stage === 2 ? 20 : 30;
  const sway =
    stage === 3 ? (
      <animateTransform
        attributeName="transform"
        type="rotate"
        values="-2 50 34;2 50 34;-2 50 34"
        dur="4s"
        repeatCount="indefinite"
      />
    ) : null;
  return (
    <svg viewBox="0 0 100 56" className="h-14 w-full" aria-hidden>
      <rect x="6" y="34" width="88" height="16" rx="4" fill="var(--surface-sunken)" />
      <g>
        {sway}
        <path d={`M50 34V${34 - h}`} stroke={color} strokeWidth="3" strokeLinecap="round" />
        {stage >= 2 && (
          <>
            <ellipse
              cx="40"
              cy={34 - h * 0.6}
              rx="9"
              ry="5"
              fill={color}
              transform={`rotate(-25 40 ${34 - h * 0.6})`}
            />
            <ellipse
              cx="60"
              cy={34 - h * 0.7}
              rx="9"
              ry="5"
              fill={color}
              transform={`rotate(25 60 ${34 - h * 0.7})`}
            />
          </>
        )}
        {stage === 3 && (
          <>
            <circle cx="50" cy={34 - h + 4} r="7" fill={color} />
            <circle cx="47.5" cy={34 - h + 1.5} r="2.2" fill="#fff" opacity=".7" />
          </>
        )}
      </g>
    </svg>
  );
}

/** 单块田：进度环 + 阶段造型 + 浇水/收获动作 */
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
        className={`flex min-h-[132px] flex-col items-center justify-between rounded-[var(--r-md)] border border-dashed p-3 text-center active:scale-[0.98] disabled:opacity-60 ${
          picking
            ? "border-sun bg-sun-soft ring-2 ring-sun"
            : "border-[var(--border-deep)] bg-[var(--surface-raised)]"
        }`}
      >
        <CropArt cropId={slot} stage={0} />
        <span className="text-xs font-bold">{t.plotEmpty.replace("{n}", String(slot))}</span>
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
  const stage: 0 | 1 | 2 | 3 = ready ? 3 : progress < 0.34 ? 1 : progress < 0.8 ? 2 : 2;
  const left = leftMs === null ? null : Math.max(0, leftMs);
  const leftText =
    left === null
      ? "…"
      : left <= 0
        ? t.mature
        : `${Math.floor(left / 3600000)}h${Math.floor((left % 3600000) / 60000)}m`;

  const r = 14;
  const c = 2 * Math.PI * r;
  return (
    <div
      className={`flex min-h-[132px] flex-col justify-between rounded-[var(--r-md)] border bg-[var(--surface-card)] p-3 shadow-[var(--shadow-card)] ${
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
              <span className="ml-1 text-[11px] font-black text-danger">🥀 {t.witheredTag}</span>
            ) : (
              ready && <span className="ml-1 text-[11px] text-mint">✓ {t.matureShort}</span>
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
            <circle cx="18" cy="18" r={r} fill="none" stroke="var(--surface-sunken)" strokeWidth="4" />
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
            className="min-h-[34px] flex-1 rounded-full bg-sky-soft px-2 text-[11px] font-bold text-ink active:scale-[0.97] disabled:opacity-50"
          >
            {t.water}
          </button>
        )}
        <button
          type="button"
          onClick={() => onHarvest(slot)}
          disabled={busy || (!ready && !withered)}
          className={`min-h-[34px] flex-1 rounded-full px-2 text-[11px] font-bold active:scale-[0.97] disabled:opacity-40 ${
            withered
              ? "border border-[var(--border-deep)] bg-[var(--surface-card)] text-sub"
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

/** 作物图鉴卡（行情） */
export function MarketCard({
  crop,
  onPlant,
  t,
}: {
  crop: Crop;
  onPlant: (cropId: number) => void;
  t: Record<string, string>;
}) {
  const pct = Math.round((crop.market_price / crop.seed_price - 1) * 100);
  const up = pct >= 0;
  const stars = Math.max(1, Math.min(3, Math.round((crop.base_yield / crop.seed_price) * 2.4)));
  return (
    <div className="rounded-[var(--r-md)] border border-line bg-[var(--surface-card)] p-3 shadow-[var(--shadow-card)]">
      <CropArt cropId={crop.id} stage={3} />
      <p className="mt-1 font-display text-sm">{crop.name}</p>
      <p className="text-[11px] text-sub">
        {t.growInfo
          .replace("{h}", String(crop.grow_hours))
          .replace("{p}", String(crop.seed_price))}
      </p>
      <p className="num mt-1 text-base font-black">{crop.market_price.toLocaleString()}</p>
      <p className={`num text-[11px] ${up ? "text-danger" : "text-mint"}`}>
        {up ? "↑ +" : "↓ "}
        {pct}% {"★".repeat(stars)}
      </p>
      <button
        type="button"
        onClick={() => onPlant(crop.id)}
        className="mt-2 min-h-[34px] w-full rounded-full bg-sun px-3 text-[11px] font-bold text-ink active:scale-[0.97]"
      >
        {t.plant}
      </button>
    </div>
  );
}
