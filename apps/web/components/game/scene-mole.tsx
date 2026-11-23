/**
 * 打地鼠机台：紫金夜空下的 3×3 洞阵——金冠鼠冒头、炸弹藏在角落，
 * 木槌正落下。大厅卡面与快捷入口共用；纯 SVG，无位图依赖。
 */
type P = { className?: string };

const HOLES: [number, number][] = [
  [54, 40], [110, 40], [166, 40],
  [54, 70], [110, 70], [166, 70],
  [54, 100], [110, 100], [166, 100],
];

function Hole({ x, y }: { x: number; y: number }) {
  return (
    <g>
      <ellipse cx={x} cy={y} rx="19" ry="8" fill="var(--border-deep)" />
      <ellipse cx={x} cy={y + 1} rx="14" ry="5.5" fill="#160f2c" />
      <path
        d={`M${x - 20} ${y - 2} Q${x} ${y - 9} ${x + 20} ${y - 2}`}
        fill="none"
        stroke="var(--sweet-gold)"
        strokeWidth="1.2"
        opacity="0.65"
      />
    </g>
  );
}

function Mouse({ x, y, crown }: { x: number; y: number; crown?: boolean }) {
  return (
    <g transform={`translate(${x} ${y})`}>
      {/* 耳朵 */}
      <circle cx="-10" cy="-20" r="7" fill="var(--candy)" />
      <circle cx="10" cy="-20" r="7" fill="var(--candy)" />
      <circle cx="-10" cy="-20" r="3.6" fill="#ffd9e8" />
      <circle cx="10" cy="-20" r="3.6" fill="#ffd9e8" />
      {/* 头 */}
      <ellipse cx="0" cy="-8" rx="13" ry="12" fill="#f7ead8" />
      <ellipse cx="0" cy="-4" rx="7" ry="5" fill="#fff7ec" />
      {/* 眼睛/鼻尖 */}
      <circle cx="-4.6" cy="-11" r="1.9" fill="#241536" />
      <circle cx="4.6" cy="-11" r="1.9" fill="#241536" />
      <circle cx="0" cy="-5.6" r="2" fill="var(--coral)" />
      {/* 胡须 */}
      <path
        d="M-2 -4 L-12 -6 M-2 -3 L-12 -1 M2 -4 L12 -6 M2 -3 L12 -1"
        stroke="#8d7fa6"
        strokeWidth="0.8"
        opacity="0.7"
      />
      {crown && (
        <path
          d="M-8 -25 L-5 -33 L0 -27 L5 -33 L8 -25 Z"
          fill="var(--sun)"
          stroke="var(--sweet-gold)"
          strokeWidth="1"
        />
      )}
    </g>
  );
}

export function SceneMole({ className }: P) {
  return (
    <svg viewBox="0 0 220 110" className={className} aria-hidden>
      <rect width="220" height="110" fill="url(#mo-hall)" />
      <defs>
        <radialGradient id="mo-hall" cx="50%" cy="26%" r="88%">
          <stop offset="0%" stopColor="var(--candy)" stopOpacity="0.55" />
          <stop offset="62%" stopColor="#4a2d7d" stopOpacity="0.45" />
          <stop offset="100%" stopColor="#160f2c" stopOpacity="0.9" />
        </radialGradient>
      </defs>

      {/* 夜空星点 */}
      {[[22, 16, 1.2], [46, 9, 0.9], [188, 13, 1.1], [204, 34, 0.8]]
        .map(([x, y, r], i) => (
          <circle
            key={i}
            cx={x}
            cy={y}
            r={r}
            fill="var(--sun)"
            opacity={i % 2 ? 0.55 : 0.9}
          />
        ))}

      {/* 洞阵 */}
      {HOLES.map(([x, y], i) => (
        <Hole key={i} x={x} y={y} />
      ))}

      {/* 金冠鼠从中间洞冒头 */}
      <ellipse cx="110" cy="86" rx="14" ry="5" fill="#160f2c" />
      <Mouse x={110} y={84} crown />
      {/* 左边灰鼠 */}
      <ellipse cx="54" cy="72" rx="12" ry="4" fill="#160f2c" />
      <Mouse x={54} y={70} />
      {/* 右下炸弹 */}
      <g transform="translate(166 87)">
        <circle r="9" fill="#2b2440" stroke="#8d7fa6" strokeWidth="1.2" />
        <circle cx="-3" cy="-3" r="2.6" fill="#fff" opacity="0.35" />
        <path
          d="M5 -7 Q10 -15 15 -12"
          fill="none"
          stroke="var(--sweet-gold)"
          strokeWidth="1.4"
        />
        <circle cx="16" cy="-12" r="2.2" fill="var(--sun)" />
        <path
          d="M14 -17 L18 -15 L14 -13"
          fill="none"
          stroke="var(--coral)"
          strokeWidth="1"
        />
      </g>

      {/* 木槌 */}
      <g transform="rotate(34 170 34)">
        <rect
          x="164"
          y="10"
          width="5"
          height="34"
          rx="2.5"
          fill="var(--sweet-gold)"
        />
        <rect
          x="150"
          y="2"
          width="34"
          height="16"
          rx="5"
          fill="var(--sun)"
          stroke="var(--sweet-gold)"
          strokeWidth="1.4"
        />
        <rect
          x="155"
          y="6"
          width="24"
          height="3"
          rx="1.5"
          fill="#fff7ec"
          opacity="0.45"
        />
      </g>

      {/* 落点冲击 */}
      <path
        d="M142 48 L134 42 M148 44 L146 35 M136 54 L127 51"
        stroke="var(--sun)"
        strokeWidth="1.6"
        opacity="0.8"
      />
      <ellipse
        cx="140"
        cy="58"
        rx="27"
        ry="11"
        fill="none"
        stroke="var(--sweet-gold)"
        strokeWidth="1.2"
        opacity="0.4"
      />

      {/* 台面金线 */}
      <line
        x1="8"
        y1="108"
        x2="212"
        y2="108"
        stroke="var(--sweet-gold)"
        strokeWidth="1"
        opacity="0.5"
        strokeDasharray="6 5"
      />
    </svg>
  );
}
