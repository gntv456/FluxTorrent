/**
 * 玩法机台画面·养成四款（波·游戏化视觉）：与 scene-a.tsx 同口径——
 * 画「这台机器在做什么」。卡面与玩法页舞台共用。
 */

type P = { className?: string };

/** 抽卡台：卡池漩涡上方悬着一张翻起的 SSR 卡 */
export function SceneGacha({ className }: P) {
  return (
    <svg viewBox="0 0 220 110" className={className} aria-hidden>
      <rect width="220" height="110" fill="url(#gc-neb)" />
      <defs>
        <radialGradient id="gc-neb" cx="50%" cy="55%" r="80%">
          <stop offset="0%" stopColor="var(--candy)" stopOpacity="0.5" />
          <stop
            offset="100%"
            stopColor="var(--indigo, var(--sky))"
            stopOpacity="0.15"
          />
        </radialGradient>
      </defs>
      {/* 漩涡 */}
      {[52, 38, 24].map((r, i) => (
        <ellipse
          key={i}
          cx="110"
          cy="66"
          rx={r * 1.6}
          ry={r * 0.62}
          fill="none"
          stroke="var(--sweet-lilac)"
          opacity={0.5 - i * 0.12}
        />
      ))}
      <circle cx="110" cy="66" r="5" fill="var(--sweet-gold)" />
      {/* 悬浮的翻起卡（金框 SSR） */}
      <g transform="rotate(-10 110 32)">
        <rect
          x="88"
          y="12"
          width="44"
          height="34"
          rx="4"
          fill="var(--surface-card)"
          stroke="var(--sweet-gold)"
          strokeWidth="2"
        />
        <text
          x="110"
          y="30"
          textAnchor="middle"
          fontSize="11"
          fontWeight="900"
          fill="var(--sun)"
        >
          SSR
        </text>
        <text x="110" y="41" textAnchor="middle" fontSize="8" fill="var(--sun)">
          ★
        </text>
      </g>
      {[
        [40, 26],
        [176, 34],
        [58, 88],
        [168, 86],
      ].map(([x, y], i) => (
        <circle
          key={`s${i}`}
          cx={x}
          cy={y}
          r="1.8"
          fill="var(--sweet-gold)"
          opacity="0.8"
        />
      ))}
    </svg>
  );
}

/** 农场：三垄田 + 一株待收金作物 + 太阳 */
export function SceneFarm({ className }: P) {
  return (
    <svg viewBox="0 0 220 110" className={className} aria-hidden>
      <rect width="220" height="110" fill="url(#fm-dawn)" />
      <defs>
        <linearGradient id="fm-dawn" x1="0" y1="0" x2="0" y2="1">
          <stop offset="0%" stopColor="var(--sun)" stopOpacity="0.3" />
          <stop offset="100%" stopColor="var(--mint)" stopOpacity="0.4" />
        </linearGradient>
      </defs>
      <circle cx="182" cy="20" r="11" fill="var(--sun)" opacity="0.9" />
      {/* 三层垄 */}
      <path
        d="M0 66 Q55 58 110 66 T220 66 V110 H0 Z"
        fill="var(--mint)"
        opacity="0.35"
      />
      <path
        d="M0 80 Q55 72 110 80 T220 80 V110 H0 Z"
        fill="var(--mint)"
        opacity="0.55"
      />
      <path
        d="M0 94 Q55 86 110 94 T220 94 V110 H0 Z"
        fill="var(--mint)"
        opacity="0.75"
      />
      {/* 待收金作物（中间垄） */}
      {[
        [70, 74],
        [110, 70],
        [150, 74],
      ].map(([x, y], i) => (
        <g key={`p${i}`}>
          <path
            d={`M${x} ${y} q-3 -10 0 -18 q3 8 0 18`}
            fill={i === 1 ? "var(--sun)" : "var(--mood-egg-mint)"}
          />
          <line
            x1={x}
            y1={y}
            x2={x}
            y2={y + 6}
            stroke="var(--sweet-gold)"
            strokeWidth="1.4"
            opacity="0.7"
          />
        </g>
      ))}
      {/* 幼苗排（近垄） */}
      {[40, 66, 92, 118, 144, 170].map((x, i) => (
        <circle
          key={`s${i}`}
          cx={x}
          cy={100}
          r="2.6"
          fill="var(--mint)"
          opacity="0.9"
        />
      ))}
    </svg>
  );
}

/** 宠物巢：软垫窝 + 一只猫 + 食盆 */
export function ScenePet({ className }: P) {
  return (
    <svg viewBox="0 0 220 110" className={className} aria-hidden>
      <rect width="220" height="110" fill="url(#pt-teal)" />
      <defs>
        <radialGradient id="pt-teal" cx="50%" cy="45%" r="85%">
          <stop
            offset="0%"
            stopColor="var(--teal, var(--mint))"
            stopOpacity="0.45"
          />
          <stop offset="100%" stopColor="var(--sky)" stopOpacity="0.15" />
        </radialGradient>
      </defs>
      {/* 窝 */}
      <ellipse
        cx="106"
        cy="66"
        rx="52"
        ry="24"
        fill="var(--surface-raised)"
        stroke="var(--sweet-gold)"
        strokeWidth="1.5"
      />
      <ellipse cx="106" cy="62" rx="42" ry="18" fill="var(--surface-card)" />
      {/* 猫（坐姿剪影+耳+眼） */}
      <g>
        <circle cx="100" cy="52" r="13" fill="var(--ink)" opacity="0.85" />
        <path d="M90 44 L92 34 L98 41 Z" fill="var(--ink)" opacity="0.85" />
        <path d="M110 44 L108 34 L102 41 Z" fill="var(--ink)" opacity="0.85" />
        <circle cx="96" cy="52" r="1.5" fill="var(--sun)" />
        <circle cx="104" cy="52" r="1.5" fill="var(--sun)" />
      </g>
      {/* 食盆 */}
      <path d="M150 74 h20 l-3 8 h-14 Z" fill="var(--coral)" opacity="0.9" />
      <ellipse
        cx="160"
        cy="74"
        rx="10"
        ry="2.5"
        fill="var(--sun)"
        opacity="0.8"
      />
      {/* 爱心 */}
      <path
        d={
          "M128 40 c-2 -4 -8 -2 -6 2 c1 3 6 6 6 6 " +
          "s5 -3 6 -6 c2 -4 -4 -6 -6 -2 Z"
        }
        fill="var(--coral)"
        opacity="0.85"
      />
    </svg>
  );
}

/** 钓鱼塘：夜塘月色 + 浮标咬钩水纹 + 鱼影 */
export function SceneFishing({ className }: P) {
  return (
    <svg viewBox="0 0 220 110" className={className} aria-hidden>
      <rect width="220" height="110" fill="url(#fs-night)" />
      <defs>
        <linearGradient id="fs-night" x1="0" y1="0" x2="0" y2="1">
          <stop
            offset="0%"
            stopColor="var(--indigo, var(--sky))"
            stopOpacity="0.5"
          />
          <stop offset="100%" stopColor="var(--sky)" stopOpacity="0.3" />
        </linearGradient>
      </defs>
      <circle cx="40" cy="20" r="9" fill="var(--sun)" opacity="0.85" />
      {/* 三层水 */}
      <path
        d="M0 58 Q28 52 55 58 T110 58 T165 58 T220 58 V110 H0 Z"
        fill="var(--sky)"
        opacity="0.4"
      />
      <path
        d="M0 72 Q28 66 55 72 T110 72 T165 72 T220 72 V110 H0 Z"
        fill="var(--sky)"
        opacity="0.55"
      />
      <path
        d="M0 86 Q28 80 55 86 T110 86 T165 86 T220 86 V110 H0 Z"
        fill="var(--sky)"
        opacity="0.7"
      />
      {/* 鱼影 */}
      <g opacity="0.5">
        <ellipse cx="90" cy="82" rx="12" ry="4.5" fill="var(--sky-deep)" />
        <path d="M102 82 l7 -4 v8 Z" fill="var(--sky-deep)" />
      </g>
      {/* 浮标 + 咬钩水纹 */}
      <rect
        x="163"
        y="44"
        width="2.5"
        height="18"
        fill="var(--ink)"
        opacity="0.7"
      />
      <circle
        cx="164"
        cy="64"
        r="5.5"
        fill="var(--coral)"
        stroke="#fff"
        strokeWidth="1"
      />
      {[24, 16].map((r, i) => (
        <circle
          key={`r${i}`}
          cx="164"
          cy="64"
          r={r}
          fill="none"
          stroke="#fff"
          strokeWidth="1"
          opacity={0.4 - i * 0.15}
        />
      ))}
    </svg>
  );
}
