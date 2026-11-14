/**
 * 舞台艺术区·续（波2）：fishing/capsule/wheel 三款——拆分自 stage-art.tsx
 * （行数门禁 300 上限），导出与其同构，消费方各自按名引。
 */

/** 水面波光（fishing）：三层波 + 浮标 */
export function ArtFishing() {
  return (
    <svg viewBox="0 0 200 90" className="gs-artsvg" aria-hidden>
      <path
        d="M0 56 Q25 50 50 56 T100 56 T150 56 T200 56 V90 H0 Z"
        fill="var(--sky)"
        opacity="0.3"
      />
      <path
        d="M0 66 Q25 60 50 66 T100 66 T150 66 T200 66 V90 H0 Z"
        fill="var(--sky)"
        opacity="0.45"
      />
      <path
        d="M0 76 Q25 70 50 76 T100 76 T150 76 T200 76 V90 H0 Z"
        fill="var(--sky)"
        opacity="0.6"
      />
      <circle cx="150" cy="58" r="4" fill="var(--coral)" />
      <rect x="149" y="48" width="2" height="8" fill="var(--coral)" />
    </svg>
  );
}

/** 蛋壳高光带（capsule）：穹顶玻璃反光 + 散落蛋影 */
export function ArtCapsule() {
  return (
    <svg viewBox="0 0 200 90" className="gs-artsvg" aria-hidden>
      <path
        d="M40 78 A60 60 0 0 1 160 78"
        fill="none"
        stroke="var(--sweet-lilac)"
        strokeWidth="1.5"
        opacity="0.6"
      />
      <path
        d="M62 46 A38 34 0 0 1 112 30"
        fill="none"
        stroke="#fff"
        strokeWidth="3"
        opacity="0.35"
        strokeLinecap="round"
      />
      {[
        [70, 72],
        [100, 76],
        [130, 72],
      ].map(([x, y], i) => (
        <ellipse
          key={i}
          cx={x}
          cy={y}
          rx="9"
          ry="7"
          fill={
            [
              "var(--mood-egg-coral)",
              "var(--mood-egg-mint)",
              "var(--mood-egg-amber)",
            ][i]
          }
          opacity="0.85"
        />
      ))}
    </svg>
  );
}

/** 中心轴徽记（wheel）：转轴 + 放射线 */
export function ArtWheel() {
  return (
    <svg viewBox="0 0 200 90" className="gs-artsvg" aria-hidden>
      <circle
        cx="100"
        cy="46"
        r="30"
        fill="none"
        stroke="var(--sweet-gold)"
        strokeWidth="1.5"
        opacity="0.5"
      />
      <circle cx="100" cy="46" r="6" fill="var(--sweet-gold)" opacity="0.8" />
      {[0, 45, 90, 135, 180, 225, 270, 315].map((a) => (
        <line
          key={a}
          x1={100 + 10 * Math.cos((a * Math.PI) / 180)}
          y1={46 + 10 * Math.sin((a * Math.PI) / 180)}
          x2={100 + 28 * Math.cos((a * Math.PI) / 180)}
          y2={46 + 28 * Math.sin((a * Math.PI) / 180)}
          stroke="var(--sweet-gold)"
          opacity="0.3"
        />
      ))}
    </svg>
  );
}

