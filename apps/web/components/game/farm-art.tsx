"use client";

/**
 * 农场作物造型与图鉴卡（从 components/game/farm-field.tsx 按域拆出，
 * 295 行门禁）：CropArt 四阶段 SVG 造型 + MarketCard 作物图鉴卡（行情）。
 * 纯搬移，无逻辑改动。
 */

export interface Crop {
  id: number;
  name: string;
  seed_price: number;
  base_yield: number;
  grow_hours: number;
  /** 下架位（0253）：false 的作物不进行情、不能播种，但已种下的地仍可收。
   *  行情列表本身只返回 active 的行，这个字段是给「图鉴里保留下架株」用的。 */
  active: boolean;
  market_price: number;
}

const CROP_COLORS = [
  "--sun",
  "--mint",
  "--candy",
  "--coral",
  "--sky",
  "--indigo",
];

/** 作物造型：空地 / 出苗 / 生长 / 成熟 四阶段（成熟带摇曳） */
export function CropArt({
  cropId,
  stage,
}: {
  cropId: number;
  stage: 0 | 1 | 2 | 3;
}) {
  const color = `var(${CROP_COLORS[(cropId - 1) % CROP_COLORS.length]})`;
  if (stage === 0) {
    return (
      <svg viewBox="0 0 100 56" className="h-14 w-full" aria-hidden>
        <rect
          x="6"
          y="34"
          width="88"
          height="16"
          rx="4"
          fill="var(--surface-sunken)"
        />
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
      <rect
        x="6"
        y="34"
        width="88"
        height="16"
        rx="4"
        fill="var(--surface-sunken)"
      />
      <g>
        {sway}
        <path
          d={`M50 34V${34 - h}`}
          stroke={color}
          strokeWidth="3"
          strokeLinecap="round"
        />
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
            <circle
              cx="47.5"
              cy={34 - h + 1.5}
              r="2.2"
              fill="#fff"
              opacity=".7"
            />
          </>
        )}
      </g>
    </svg>
  );
}

const MARKET_CARD_CLS =
  "rounded-[var(--r-md)] border border-line bg-[var(--surface-card)] " +
  "p-3 shadow-[var(--shadow-card)]";

const PLANT_BTN_CLS =
  "mt-2 min-h-[34px] w-full rounded-full bg-sun px-3 text-[11px] " +
  "font-bold text-ink active:scale-[0.97]";

/** 作物图鉴卡（行情） */
export function MarketCard({
  crop,
  onPlant,
  busy,
  t,
}: {
  crop: Crop;
  onPlant: (cropId: number) => void;
  /** 播种请求进行中：防连点（与田块按钮同纪律） */
  busy?: boolean;
  t: Record<string, string>;
}) {
  const pct = Math.round((crop.market_price / crop.seed_price - 1) * 100);
  const up = pct >= 0;
  const stars = Math.max(
    1,
    Math.min(3, Math.round((crop.base_yield / crop.seed_price) * 2.4)),
  );
  return (
    <div className={MARKET_CARD_CLS}>
      <CropArt cropId={crop.id} stage={3} />
      <p className="mt-1 font-display text-sm">{crop.name}</p>
      <p className="text-[11px] text-sub">
        {t.growInfo
          .replace("{h}", String(crop.grow_hours))
          .replace("{p}", String(crop.seed_price))}
      </p>
      <p className="num mt-1 text-base font-black">
        {crop.market_price.toLocaleString()}
      </p>
      <p className={`num text-[11px] ${up ? "text-danger" : "text-mint"}`}>
        {up ? "↑ +" : "↓ "}
        {pct}% {"★".repeat(stars)}
      </p>
      <button
        type="button"
        onClick={() => onPlant(crop.id)}
        disabled={busy}
        className={PLANT_BTN_CLS}
      >
        {t.plant}
      </button>
    </div>
  );
}
