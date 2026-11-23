/**
 * 玩法机台画面·续（波·游戏化视觉）：扭蛋机 + 大转盘——从 scene-a 拆出
 * （行数门禁 300 上限），与 scene-a/scene-b 同口径。
 */

type P = { className?: string };

/** 大转盘：八扇转盘指针停在金扇，灯泡环闪烁 */
export function SceneWheel({ className }: P) {
  const cx = 110,
    cy = 58,
    R = 40;
  const gold = [0, 2, 5];
  return (
    <svg viewBox="0 0 220 110" className={className} aria-hidden>
      <rect width="220" height="110" fill="url(#wh-dusk)" />
      <defs>
        <linearGradient id="wh-dusk" x1="0" y1="0" x2="0" y2="1">
          <stop offset="0%" stopColor="var(--sun)" stopOpacity="0.4" />
          <stop offset="100%" stopColor="var(--coral)" stopOpacity="0.2" />
        </linearGradient>
      </defs>
      {Array.from({ length: 8 }, (_, i) => {
        const a0 = (i * 45 - 90) * (Math.PI / 180);
        const a1 = ((i + 1) * 45 - 90) * (Math.PI / 180);
        const p = (a: number, k: number) =>
          `${cx + R * Math.cos(a) * k} ${cy + R * Math.sin(a) * k}`;
        const d = `M${cx} ${cy} L${p(a0, 1)} A${R} ${R} 0 0 1 ${p(a1, 1)} Z`;
        return (
          <path
            key={i}
            d={d}
            fill={gold.includes(i) ? "var(--sun)" : "var(--surface-card)"}
            opacity={gold.includes(i) ? 0.95 : 0.75}
            stroke="var(--sweet-gold)"
            strokeWidth="1"
          />
        );
      })}
      <circle
        cx={cx}
        cy={cy}
        r="8"
        fill="var(--surface-raised)"
        stroke="var(--sweet-gold)"
        strokeWidth="2"
      />
      {/* 指针（顶部） */}
      <path
        d="M110 8 L104 22 L116 22 Z"
        fill="var(--coral)"
        stroke="var(--sweet-gold)"
        strokeWidth="1"
      />
      {/* 灯泡环 */}
      {Array.from({ length: 10 }, (_, i) => {
        const a = i * 36 * (Math.PI / 180);
        return (
          <circle
            key={`l${i}`}
            cx={cx + (R + 8) * Math.cos(a)}
            cy={cy + (R + 8) * Math.sin(a)}
            r="2.4"
            fill={i % 2 ? "var(--sun)" : "var(--surface-card)"}
            opacity={i % 2 ? 0.95 : 0.5}
          />
        );
      })}
    </svg>
  );
}

/** 老虎机：机身 + 三滚轮窗 + 顶部灯排 + 右拉杆；中窗停在红 7 */
export function SceneSlot({ className }: P) {
  return (
    <svg viewBox="0 0 220 110" className={className} aria-hidden>
      <rect width="220" height="110" fill="url(#sl-indigo)" />
      <defs>
        <linearGradient id="sl-indigo" x1="0" y1="0" x2="0" y2="1">
          <stop
            offset="0%"
            stopColor="var(--indigo, var(--sky))"
            stopOpacity="0.5"
          />
          <stop offset="100%" stopColor="var(--coral)" stopOpacity="0.18" />
        </linearGradient>
      </defs>
      {/* 机身 */}
      <rect
        x="56"
        y="28"
        width="108"
        height="58"
        rx="10"
        fill="var(--surface-raised)"
        stroke="var(--sweet-gold)"
        strokeWidth="2"
      />
      {/* 顶部灯排 */}
      {Array.from({ length: 5 }, (_, i) => (
        <circle
          key={i}
          cx={70 + i * 27}
          cy="34"
          r="2.6"
          fill={i % 2 ? "var(--sun)" : "var(--surface-card)"}
          opacity={i % 2 ? 0.95 : 0.55}
        />
      ))}
      {/* 三滚轮窗 */}
      {[70, 101, 132].map((x, i) => (
        <rect
          key={i}
          x={x}
          y="42"
          width="24"
          height="30"
          rx="5"
          fill="var(--surface-card)"
          stroke="var(--sweet-lilac)"
        />
      ))}
      <text
        x="113"
        y="64"
        textAnchor="middle"
        fontSize="15"
        fontWeight="700"
        fill="var(--coral)"
      >
        7
      </text>
      <circle cx="82" cy="57" r="4" fill="var(--sun)" opacity="0.85" />
      <path d="M138 54l4 6-8 0Z" fill="var(--sky)" opacity="0.85" />
      {/* 拉杆 */}
      <line
        x1="164"
        y1="40"
        x2="164"
        y2="58"
        stroke="var(--sweet-gold)"
        strokeWidth="2.4"
      />
      <circle
        cx="164"
        cy="38"
        r="5"
        fill="var(--coral)"
        stroke="var(--sweet-gold)"
      />
    </svg>
  );
}

/** 水果机：蓝面板 + 中央樱桃窗 + 两侧水果列 + 灯排 */
export function SceneFruit({ className }: P) {
  return (
    <svg viewBox="0 0 220 110" className={className} aria-hidden>
      <rect width="220" height="110" fill="url(#fr-blue)" />
      <defs>
        <linearGradient id="fr-blue" x1="0" y1="0" x2="0" y2="1">
          <stop offset="0%" stopColor="var(--sky)" stopOpacity="0.55" />
          <stop offset="100%" stopColor="var(--mint)" stopOpacity="0.2" />
        </linearGradient>
      </defs>
      <rect
        x="48"
        y="24"
        width="124"
        height="62"
        rx="10"
        fill="var(--surface-raised)"
        stroke="var(--sweet-gold)"
        strokeWidth="2"
      />
      {/* 顶部灯排 */}
      {Array.from({ length: 7 }, (_, i) => (
        <circle
          key={i}
          cx={60 + i * 17}
          cy="30"
          r="2.2"
          fill={i % 2 ? "var(--coral)" : "var(--sun)"}
          opacity={i % 2 ? 0.95 : 0.55}
        />
      ))}
      {/* 中央果窗 */}
      <circle
        cx="110"
        cy="58"
        r="17"
        fill="var(--surface-card)"
        stroke="var(--sweet-lilac)"
      />
      <circle cx="104" cy="61" r="5.5" fill="var(--coral)" />
      <circle cx="115" cy="61" r="5.5" fill="var(--coral)" />
      <path
        d="M110 45c1.5 2 3.4 3 6 3.3"
        fill="none"
        stroke="var(--mint)"
        strokeWidth="1.6"
      />
      {/* 两侧水果 */}
      <circle cx="68" cy="52" r="5" fill="var(--sun)" opacity="0.9" />
      <circle cx="68" cy="68" r="5" fill="var(--mint)" opacity="0.9" />
      <circle cx="152" cy="52" r="5" fill="var(--sky)" opacity="0.9" />
      <circle cx="152" cy="68" r="5" fill="var(--coral)" opacity="0.9" />
    </svg>
  );
}
