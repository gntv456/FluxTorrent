import { prizeGlyph, prizeIcon, prizeValue } from "@/lib/wheel-prize";
import type { WheelPrize } from "@/lib/wheel-prize";

const VIEW = 360;
const CX = VIEW / 2;
const CY = VIEW / 2;
const R = 150;

/** 极坐标取点：0° 在正上方、顺时针为正。 */
function polar(deg: number, r: number): [number, number] {
  const a = ((deg - 90) * Math.PI) / 180;
  return [CX + r * Math.cos(a), CY + r * Math.sin(a)];
}

/** 扇形路径：圆心 → 起弧 → 顺弧 → 回圆心。 */
function sector(a1: number, a2: number): string {
  const [x1, y1] = polar(a1, R);
  const [x2, y2] = polar(a2, R);
  const large = a2 - a1 > 180 ? 1 : 0;
  return `M ${CX} ${CY} L ${x1} ${y1} A ${R} ${R} 0 ${large} 1
    ${x2} ${y2} Z`.replace(/\s+/g, " ");
}

/** 档位配色：大奖金 / 高档紫 / 中档蓝 / 小档雾白，相邻交错避免连片。 */
function segTone(v: number, ticket: number): string {
  if (v >= ticket * 20) return "Gold";
  if (v >= ticket * 3) return "Violet";
  if (v >= ticket) return "Blue";
  return "Mist";
}

function segFill(v: number, ticket: number): string {
  return `url(#lwSeg${segTone(v, ticket)})`;
}

/** 四档渐变的 id 与起止色令牌（浅彩盘面，落在紫夜舞台上）。 */
const SEG_GRAD = [
  ["Gold", "--mood-wheel-gold-a", "--mood-wheel-gold-b"],
  ["Violet", "--mood-wheel-violet-a", "--mood-wheel-violet-b"],
  ["Blue", "--mood-wheel-blue-a", "--mood-wheel-blue-b"],
  ["Mist", "--mood-wheel-mist-a", "--mood-wheel-mist-b"],
] as const;

/**
 * 纯盘面：金圈 + 灯珠 + N 个扇区（图标 + 读数）+ 命中高亮。
 * 转动与停格由 lucky-wheel 的 transform 负责，这里不持状态。
 * N 到 18 档时自动缩小图标/字号，不再另开一套密盘样式。
 */
export function WheelDisc({
  prizes,
  ticket,
  hitIndex,
}: {
  prizes: WheelPrize[];
  ticket: number;
  hitIndex: number | null;
}) {
  const n = prizes.length;
  const seg = n ? 360 / n : 360;
  const dense = n > 12;
  const iconSize = dense ? 12.5 : 18;
  const glyphSize = dense ? 10 : 13;
  const lampN = Math.max(12, n);

  return (
    <svg
      className="lw-disc"
      viewBox={`0 0 ${VIEW} ${VIEW}`}
      aria-hidden
    >
      <defs>
        <radialGradient id="lwRim" cx="0.5" cy="0.3" r="0.8">
          <stop offset="0%" style={{ stopColor: "var(--mood-wheel-rim-a)" }} />
          <stop
            offset="100%"
            style={{ stopColor: "var(--mood-wheel-rim-b)" }}
          />
        </radialGradient>
        {SEG_GRAD.map(([tone, a, b]) => (
          <radialGradient
            key={tone}
            id={`lwSeg${tone}`}
            cx="0.5"
            cy="0.35"
            r="0.85"
          >
            <stop offset="0%" style={{ stopColor: `var(${a})` }} />
            <stop offset="100%" style={{ stopColor: `var(${b})` }} />
          </radialGradient>
        ))}
      </defs>

      <circle cx={CX} cy={CY} r={R + 18} fill="url(#lwRim)" />
      <circle
        cx={CX}
        cy={CY}
        r={R + 11}
        fill="none"
        style={{ stroke: "var(--mood-wheel-disc-line)" }}
        strokeWidth="2"
        opacity="0.75"
      />
      {Array.from({ length: lampN }).map((_, i) => {
        const [lx, ly] = polar((i * 360) / lampN, R + 11);
        const gold = i % 2 === 0;
        return (
          <circle
            key={i}
            cx={lx}
            cy={ly}
            r={4}
            style={{
              fill: gold
                ? "var(--mood-wheel-lamp)"
                : "var(--mood-wheel-lamp-b)",
            }}
            stroke="var(--mood-wheel-rim-b)"
            strokeWidth="1"
          />
        );
      })}
      <circle
        cx={CX}
        cy={CY}
        r={R + 5}
        style={{ fill: "var(--mood-wheel-disc-bg)" }}
        opacity="0.9"
      />

      {prizes.map((p, i) => {
        const a1 = i * seg;
        const mid = a1 + seg / 2;
        const [ix, iy] = polar(mid, R * 0.55);
        const [tx, ty] = polar(mid, R * 0.8);
        const v = prizeValue(p, ticket);
        const hit = hitIndex === i;
        return (
          <g key={i}>
            <path
              d={sector(a1, a1 + seg)}
              style={{ fill: segFill(v, ticket) }}
              stroke={
                hit
                  ? "var(--mood-wheel-lamp)"
                  : "var(--mood-wheel-disc-line)"
              }
              strokeWidth={hit ? 3.5 : 1.6}
              opacity="0.98"
            />
            {p.image_url ? (
              <image
                href={p.image_url}
                x={ix - iconSize / 2}
                y={iy - iconSize / 2}
                width={iconSize}
                height={iconSize}
                preserveAspectRatio="xMidYMid meet"
              />
            ) : (
              <text
                x={ix}
                y={iy}
                textAnchor="middle"
                dominantBaseline="middle"
                fontSize={iconSize}
              >
                {prizeIcon(p, ticket)}
              </text>
            )}
            <text
              x={tx}
              y={ty}
              textAnchor="middle"
              dominantBaseline="middle"
              fontSize={glyphSize}
              fontWeight="800"
              style={{
                fill:
                  v >= ticket * 10
                    ? "var(--mood-wheel-text-gold)"
                    : "var(--mood-wheel-disc-ink)",
                fontVariantNumeric: "tabular-nums",
              }}
            >
              {prizeGlyph(p)}
            </text>
          </g>
        );
      })}

      <circle
        cx={CX}
        cy={CY}
        r={R}
        fill="none"
        stroke="rgba(255,255,255,0.6)"
        strokeWidth="2"
      />
    </svg>
  );
}
