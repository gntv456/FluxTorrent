"use client";

import { useMediaQuery } from "@/lib/hooks/use-media";

export type FishPhase =
  "idle" | "casting" | "waiting" | "bite" | "reeling" | "done";

/** 星夜钓场摆位表：竖版（<1024，手机全出血样图⑩）与横版（桌面双栏
 *  16:10）共用一套元素画法，只换坐标——鱼群洄游 keyframes 随版式
 *  各配一套（区间按各自摆位 x 算，起止都在画外）。竖版比例与
 *  arcade-sweet.css 的 .fp-pond-svg 宽度公式（×0.8214）、横版与
 *  .fp-wide（×1.6）成对，改 viewBox 须同步那两个系数。 */
const SCENE_TALL = {
  w: 460,
  h: 560,
  waterTop: 224,
  moon: { cx: 390, cy: 55, r: 25 },
  stars: [
    [40, 30, 1.5],
    [120, 50, 1],
    [200, 25, 1.2],
    [350, 40, 1.5],
    [420, 60, 1],
  ],
  waves: [252, 308, 364],
  weeds: [
    { d: "M 30 545 Q 35 430 25 340", c: "#2d6a4f", sw: 4 },
    { d: "M 45 545 Q 50 450 42 360", c: "#40916c", sw: 3 },
    { d: "M 420 545 Q 415 440 425 350", c: "#2d6a4f", sw: 4 },
    { d: "M 405 545 Q 400 460 408 370", c: "#40916c", sw: 3 },
  ],
  bubbles: [
    [80, 400, 3],
    [90, 450, 2],
    [380, 420, 3],
    [370, 470, 2],
  ],
  fish: [
    { cls: "fp-f1", kind: "blue" as const, x: 110, y: 390 },
    { cls: "fp-f2", kind: "orange" as const, x: 330, y: 470 },
    { cls: "fp-f3", kind: "far" as const, x: 190, y: 320 },
    { cls: "fp-f4", kind: "tiny" as const, x: 290, y: 300 },
  ],
  lineBottom: 308,
};

const SCENE_WIDE = {
  w: 640,
  h: 400,
  waterTop: 150,
  moon: { cx: 560, cy: 52, r: 24 },
  stars: [
    [40, 30, 1.5],
    [130, 55, 1],
    [230, 25, 1.2],
    [380, 40, 1.5],
    [460, 62, 1],
    [612, 34, 1],
  ],
  waves: [170, 200, 230],
  weeds: [
    { d: "M 40 390 Q 45 300 35 225", c: "#2d6a4f", sw: 4 },
    { d: "M 55 390 Q 60 315 52 245", c: "#40916c", sw: 3 },
    { d: "M 600 390 Q 595 305 605 230", c: "#2d6a4f", sw: 4 },
    { d: "M 585 390 Q 580 320 588 250", c: "#40916c", sw: 3 },
  ],
  bubbles: [
    [95, 270, 3],
    [105, 305, 2],
    [545, 285, 3],
    [535, 320, 2],
  ],
  fish: [
    { cls: "fp-f1", kind: "blue" as const, x: 145, y: 265 },
    { cls: "fp-f2", kind: "orange" as const, x: 400, y: 330 },
    { cls: "fp-f3", kind: "far" as const, x: 250, y: 215 },
    { cls: "fp-f4", kind: "tiny" as const, x: 330, y: 195 },
  ],
  lineBottom: 210,
};

type FishKind = "blue" | "orange" | "far" | "tiny";

/** 鱼体画法（坐标以鱼心为原点，摆位由外层 translate 控制） */
function FishBody({ kind }: { kind: FishKind }) {
  if (kind === "blue")
    return (
      <>
        <ellipse cx="0" cy="0" rx="18" ry="8" fill="#5a9fd4" />
        <polygon points="-18,0 -28,-6 -28,6" fill="#4a8fc4" />
        <circle cx="8" cy="-2" r="2" fill="#fff" />
        <circle cx="9" cy="-2" r="1" fill="#000" />
      </>
    );
  if (kind === "orange")
    return (
      <>
        <ellipse cx="0" cy="0" rx="22" ry="10" fill="#e8945a" />
        <polygon points="-22,0 -34,-8 -34,8" fill="#d8844a" />
        <circle cx="10" cy="-3" r="2.5" fill="#fff" />
        <circle cx="11" cy="-3" r="1.2" fill="#000" />
        <line
          x1="-5"
          y1="-8"
          x2="-5"
          y2="8"
          stroke="#c8743a"
          strokeWidth="1.5"
        />
        <line x1="0" y1="-9" x2="0" y2="9" stroke="#c8743a" strokeWidth="1.5" />
      </>
    );
  if (kind === "far")
    return (
      <>
        <ellipse cx="0" cy="0" rx="10" ry="4" fill="#8ab4dd" opacity="0.6" />
        <polygon points="-10,0 -16,-3 -16,3" fill="#7aa4cd" opacity="0.6" />
      </>
    );
  return (
    <>
      <ellipse cx="0" cy="0" rx="8" ry="3.5" fill="#b8a8d8" opacity="0.5" />
      <polygon points="-8,0 -13,-2.5 -13,2.5" fill="#a898c8" opacity="0.5" />
    </>
  );
}

/**
 * 鱼塘舞台：精致星夜钓场 SVG（月亮 + 分层水面 + 水草 + 气泡 + 鱼群 + 浮漂）。
 * 计时逻辑在页面（它持有定时器），这里只按 phase 画状态。
 * 版式：≥1024 横版 16:10（配合 CSS 高度上限，塘不再顶穿首屏），
 * 以下竖版全出血。SSR 首帧竖版，挂载后校正（use-media 纪律）。
 */
export function FishingPond({
  phase,
  windowPct,
  waitingLabel,
  biteLabel,
}: {
  phase: FishPhase;
  /** 起竿窗口剩余比例（0..1），仅 bite 阶段有意义 */
  windowPct: number;
  waitingLabel: string;
  biteLabel: string;
}) {
  const inWater = phase === "waiting" || phase === "bite";
  const wide = useMediaQuery("(min-width: 1024px)", false);
  const sc = wide ? SCENE_WIDE : SCENE_TALL;

  return (
    <div className={`fp-pond-svg${wide ? " fp-wide" : ""}`}>
      <svg
        viewBox={`0 0 ${sc.w} ${sc.h}`}
        style={{ width: "100%", height: "auto", display: "block" }}
        aria-hidden
      >
        <defs>
          <linearGradient id="skyG" x1="0" y1="0" x2="0" y2="1">
            <stop offset="0%" stopColor="#0d1b3e" />
            <stop offset="100%" stopColor="#1a3a6c" />
          </linearGradient>
          <linearGradient id="waterG" x1="0" y1="0" x2="0" y2="1">
            <stop offset="0%" stopColor="#2a5a9c" />
            <stop offset="100%" stopColor="#0d2a5c" />
          </linearGradient>
          <radialGradient id="moonG" cx="0.4" cy="0.4" r="0.6">
            <stop offset="0%" stopColor="#fff8e0" />
            <stop offset="100%" stopColor="#e8d8a0" />
          </radialGradient>
        </defs>

        {/* 夜空 */}
        <rect x="0" y="0" width={sc.w} height={sc.waterTop} fill="url(#skyG)" />
        {/* 星星（闪烁相位错开，class 轮转 fp-s1..s5） */}
        {sc.stars.map(([cx, cy, r], i) => (
          <circle
            key={i}
            className={`fp-star fp-s${(i % 5) + 1}`}
            cx={cx}
            cy={cy}
            r={r}
            fill="#fff"
            opacity="0.8"
          />
        ))}
        {/* 月亮 */}
        <circle
          cx={sc.moon.cx}
          cy={sc.moon.cy}
          r={sc.moon.r}
          fill="url(#moonG)"
        />
        <circle
          cx={sc.moon.cx - 8}
          cy={sc.moon.cy - 5}
          r={sc.moon.r - 3}
          fill="#0d1b3e"
          opacity="0.15"
        />

        {/* 水面 */}
        <rect
          x="0"
          y={sc.waterTop}
          width={sc.w}
          height={sc.h - sc.waterTop}
          fill="url(#waterG)"
        />
        {/* 水面波光横线（呼吸明灭） */}
        {sc.waves.map((y, i) => (
          <line
            key={i}
            className={`fp-wave fp-w${i + 1}`}
            x1="0"
            y1={y}
            x2={sc.w}
            y2={y}
            stroke={`rgba(255,255,255,${0.15 - i * 0.035})`}
            strokeWidth="1"
          />
        ))}

        {/* 水草（根部摇曳） */}
        {sc.weeds.map((wd, i) => (
          <path
            key={i}
            className={`fp-weed fp-wd${i + 1}`}
            d={wd.d}
            stroke={wd.c}
            strokeWidth={wd.sw}
            fill="none"
            strokeLinecap="round"
          />
        ))}

        {/* 气泡（上浮消散，相位错开） */}
        {sc.bubbles.map(([cx, cy, r], i) => (
          <circle
            key={i}
            className={`fp-bub fp-b${i + 1}`}
            cx={cx}
            cy={cy}
            r={r}
            fill="rgba(255,255,255,0.3)"
          />
        ))}

        {/* 鱼群（外层 g 只吃 CSS 洄游动画，内层保持摆位） */}
        {sc.fish.map((f) => (
          <g className={`fp-fish ${f.cls}`} key={f.cls}>
            <g transform={`translate(${f.x},${f.y})`}>
              <FishBody kind={f.kind} />
            </g>
          </g>
        ))}

        {/* 浮漂（钓鱼线 + 浮标） */}
        {inWater && (
          <g>
            {/* 钓鱼线 */}
            <line
              x1={sc.w / 2}
              y1={sc.waterTop}
              x2={sc.w / 2}
              y2={sc.lineBottom}
              stroke="#fff"
              strokeWidth="1"
              opacity="0.6"
            />
            {/* 浮漂：待机随波轻浮，咬钩抖动 */}
            <circle
              cx={sc.w / 2}
              cy={sc.lineBottom}
              r={phase === "bite" ? 8 : 6}
              fill="#e74c3c"
              stroke="#fff"
              strokeWidth="2"
              className={phase === "bite" ? "fp-bobber fp-bite" : "fp-bobber"}
            />
          </g>
        )}
      </svg>

      {/* HUD 标签 */}
      <div className="fp-hud">
        <span className="fp-label">
          {phase === "bite" ? biteLabel : waitingLabel}
        </span>
        {phase === "bite" && (
          <span className="fp-window" aria-hidden>
            <i style={{ width: `${Math.max(0, windowPct) * 100}%` }} />
          </span>
        )}
      </div>
    </div>
  );
}
