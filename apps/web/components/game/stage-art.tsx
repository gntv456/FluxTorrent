/**
 * 舞台艺术区（波2 视觉层）：8 款玩法各一块内联 SVG——零位图资源依赖
 * （仓库口径：emoji/SVG，不引图片资产），全部走 TIDE/甜梦令牌上色。
 * 用法：作为 GameStage 的 children 底层装饰层，或独立挂在无舞台页
 * （bigsmall/scratch/gacha）的 stage 顶部。
 * 纯静态 SVG（无 hooks、无 "use client"）：reduced-motion 不需要降级——
 * 本组件不含动画；动效由 arcade-fx.css 的既有规则承担。
 */

/** 骰盅质感盘（bigsmall）：木纹圆盘 + 三骰影 + 中线 */
export function ArtBigsmall() {
  return (
    <svg viewBox="0 0 200 90" className="gs-artsvg" aria-hidden>
      <ellipse cx="100" cy="58" rx="86" ry="24" fill="var(--surface-sunken)" />
      <ellipse
        cx="100"
        cy="54"
        rx="86"
        ry="24"
        fill="var(--mood-tier-gold)"
        opacity="0.28"
      />
      <ellipse
        cx="100"
        cy="54"
        rx="86"
        ry="24"
        fill="none"
        stroke="var(--sweet-gold)"
        strokeWidth="1.5"
      />
      <line
        x1="34"
        y1="54"
        x2="166"
        y2="54"
        stroke="var(--sweet-gold)"
        strokeDasharray="4 5"
        opacity="0.6"
      />
      {[58, 100, 142].map((x, i) => (
        <rect
          key={i}
          x={x - 9}
          y={38}
          width="18"
          height="18"
          rx="4"
          fill="#fff"
          stroke="var(--sweet-gold)"
          transform={`rotate(${i * 12 - 10} ${x} 47)`}
        />
      ))}
    </svg>
  );
}

/** 信封火漆纹（scratch）：信封折线 + 火漆印 + 星尘 */
export function ArtScratch() {
  return (
    <svg viewBox="0 0 200 90" className="gs-artsvg" aria-hidden>
      <rect
        x="30"
        y="18"
        width="140"
        height="58"
        rx="6"
        fill="var(--surface-card)"
        stroke="var(--sweet-gold)"
      />
      <path
        d="M30 24 L100 52 L170 24"
        fill="none"
        stroke="var(--sweet-gold)"
        strokeWidth="1.5"
      />
      <circle
        cx="100"
        cy="52"
        r="11"
        fill="var(--mood-tier-gold)"
        opacity="0.85"
      />
      <text
        x="100"
        y="57"
        textAnchor="middle"
        fontSize="11"
        fill="#fff"
        fontStyle="italic"
        fontWeight="700"
      >
        ✦
      </text>
      {[
        [46, 30],
        [158, 34],
        [140, 68],
        [58, 66],
      ].map(([x, y], i) => (
        <circle
          key={i}
          cx={x}
          cy={y}
          r="1.6"
          fill="var(--sweet-gold)"
          opacity="0.7"
        />
      ))}
    </svg>
  );
}

/** 卡池星云（gacha）：漩涡星轨 + 卡形剪影 */
export function ArtGacha() {
  return (
    <svg viewBox="0 0 200 90" className="gs-artsvg" aria-hidden>
      <ellipse
        cx="100"
        cy="46"
        rx="70"
        ry="26"
        fill="none"
        stroke="var(--sweet-lilac)"
        opacity="0.5"
      />
      <ellipse
        cx="100"
        cy="46"
        rx="46"
        ry="17"
        fill="none"
        stroke="var(--sweet-lilac)"
        opacity="0.35"
      />
      <circle cx="100" cy="46" r="4" fill="var(--sweet-gold)" />
      {[
        [52, 34],
        [148, 56],
        [86, 22],
        [120, 70],
      ].map(([x, y], i) => (
        <circle
          key={i}
          cx={x}
          cy={y}
          r="1.8"
          fill="var(--sweet-gold)"
          opacity="0.8"
        />
      ))}
      <rect
        x="86"
        y="34"
        width="28"
        height="24"
        rx="3"
        fill="var(--surface-card)"
        stroke="var(--sweet-gold)"
        transform="rotate(-8 100 46)"
      />
    </svg>
  );
}

/** 田垄景深（farm）：三层垄线 + 月 */
export function ArtFarm() {
  return (
    <svg viewBox="0 0 200 90" className="gs-artsvg" aria-hidden>
      <circle cx="160" cy="20" r="10" fill="var(--sun)" opacity="0.55" />
      <path
        d="M0 62 Q50 54 100 62 T200 62 V90 H0 Z"
        fill="var(--mint)"
        opacity="0.35"
      />
      <path
        d="M0 72 Q50 64 100 72 T200 72 V90 H0 Z"
        fill="var(--mint)"
        opacity="0.5"
      />
      <path
        d="M0 82 Q50 74 100 82 T200 82 V90 H0 Z"
        fill="var(--mint)"
        opacity="0.65"
      />
      {[40, 90, 140].map((x, i) => (
        <path
          key={i}
          d={`M${x} 60 q3 -12 0 -16 q-3 4 0 16`}
          fill="var(--mood-egg-mint)"
          stroke="none"
          opacity="0.9"
        />
      ))}
    </svg>
  );
}

/** 巢穴背景（pet）：编织巢圈 + 爪印 */
export function ArtPet() {
  return (
    <svg viewBox="0 0 200 90" className="gs-artsvg" aria-hidden>
      <ellipse
        cx="100"
        cy="56"
        rx="60"
        ry="22"
        fill="none"
        stroke="var(--sweet-gold)"
        strokeWidth="2"
        opacity="0.5"
      />
      <ellipse
        cx="100"
        cy="56"
        rx="48"
        ry="17"
        fill="none"
        stroke="var(--sweet-gold)"
        strokeWidth="1.2"
        strokeDasharray="6 4"
        opacity="0.4"
      />
      {[
        [70, 40],
        [100, 32],
        [130, 40],
      ].map(([x, y], i) => (
        <g key={i} opacity="0.55">
          <circle cx={x} cy={y} r="3" fill="var(--sweet-gold)" />
          <circle cx={x - 4} cy={y - 4} r="1.2" fill="var(--sweet-gold)" />
          <circle cx={x} cy={y - 5} r="1.2" fill="var(--sweet-gold)" />
          <circle cx={x + 4} cy={y - 4} r="1.2" fill="var(--sweet-gold)" />
        </g>
      ))}
    </svg>
  );
}
