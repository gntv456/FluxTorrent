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

/** 扭蛋机：玻璃穹顶 + 彩蛋堆 + 出蛋口正掉出一颗 */
export function SceneCapsule({ className }: P) {
  return (
    <svg viewBox="0 0 220 110" className={className} aria-hidden>
      <rect width="220" height="110" fill="url(#cp-violet)" />
      <defs>
        <linearGradient id="cp-violet" x1="0" y1="0" x2="0" y2="1">
          <stop offset="0%" stopColor="var(--candy)" stopOpacity="0.5" />
          <stop
            offset="100%"
            stopColor="var(--indigo, var(--sky))"
            stopOpacity="0.2"
          />
        </linearGradient>
      </defs>
      {/* 穹顶 */}
      <path
        d="M66 74 A44 44 0 0 1 154 74 Z"
        fill="var(--surface-card)"
        opacity="0.55"
        stroke="var(--sweet-lilac)"
        strokeWidth="2"
      />
      <path
        d="M84 38 A26 26 0 0 1 122 34"
        fill="none"
        stroke="#fff"
        strokeWidth="4"
        strokeLinecap="round"
        opacity="0.5"
      />
      {/* 蛋堆 */}
      {[
        [86, 66, "var(--coral)"],
        [104, 62, "var(--mint)"],
        [122, 66, "var(--sun)"],
        [95, 52, "var(--sky)"],
        [113, 52, "var(--coral)"],
      ].map(([x, y, c], i) => (
        <ellipse
          key={i}
          cx={x as number}
          cy={y as number}
          rx="10"
          ry="8"
          fill={c as string}
          opacity="0.9"
        />
      ))}
      {/* 机身 + 出蛋口 */}
      <rect
        x="66"
        y="74"
        width="88"
        height="20"
        rx="6"
        fill="var(--surface-raised)"
        stroke="var(--sweet-lilac)"
      />
      <circle
        cx="110"
        cy="98"
        r="7"
        fill="var(--surface-card)"
        stroke="var(--sweet-gold)"
        strokeWidth="1.5"
      />
      <ellipse cx="110" cy="97" rx="5" ry="4" fill="var(--sun)" />
      {/* 旋钮 */}
      <circle
        cx="168"
        cy="60"
        r="9"
        fill="var(--surface-raised)"
        stroke="var(--sweet-gold)"
        strokeWidth="1.5"
      />
      <line
        x1="168"
        y1="60"
        x2="174"
        y2="54"
        stroke="var(--sweet-gold)"
        strokeWidth="2"
      />
    </svg>
  );
}
