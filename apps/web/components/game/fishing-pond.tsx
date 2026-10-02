"use client";

export type FishPhase =
  "idle" | "casting" | "waiting" | "bite" | "reeling" | "done";

/**
 * 鱼塘舞台：精致星夜钓场 SVG（月亮 + 分层水面 + 水草 + 气泡 + 鱼群 + 浮漂）。
 * 计时逻辑在页面（它持有定时器），这里只按 phase 画状态。
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
  const VIEW_W = 460;
  const VIEW_H = 560;

  return (
    <div className="fp-pond-svg">
      <svg
        viewBox={`0 0 ${VIEW_W} ${VIEW_H}`}
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
        <rect
          x="0"
          y="0"
          width={VIEW_W}
          height={VIEW_H * 0.4}
          fill="url(#skyG)"
        />
        {/* 星星（闪烁相位错开） */}
        <circle
          className="fp-star fp-s1"
          cx="40"
          cy="30"
          r="1.5"
          fill="#fff"
          opacity="0.8"
        />
        <circle
          className="fp-star fp-s2"
          cx="120"
          cy="50"
          r="1"
          fill="#fff"
          opacity="0.6"
        />
        <circle
          className="fp-star fp-s3"
          cx="200"
          cy="25"
          r="1.2"
          fill="#fff"
          opacity="0.7"
        />
        <circle
          className="fp-star fp-s4"
          cx="350"
          cy="40"
          r="1.5"
          fill="#fff"
          opacity="0.8"
        />
        <circle
          className="fp-star fp-s5"
          cx="420"
          cy="60"
          r="1"
          fill="#fff"
          opacity="0.5"
        />
        {/* 月亮 */}
        <circle cx={VIEW_W - 70} cy="55" r="25" fill="url(#moonG)" />
        <circle cx={VIEW_W - 78} cy="50" r="22" fill="#0d1b3e" opacity="0.15" />

        {/* 水面 */}
        <rect
          x="0"
          y={VIEW_H * 0.4}
          width={VIEW_W}
          height={VIEW_H * 0.6}
          fill="url(#waterG)"
        />
        {/* 水面波光横线（呼吸明灭） */}
        <line
          className="fp-wave fp-w1"
          x1="0"
          y1={VIEW_H * 0.45}
          x2={VIEW_W}
          y2={VIEW_H * 0.45}
          stroke="rgba(255,255,255,0.15)"
          strokeWidth="1"
        />
        <line
          className="fp-wave fp-w2"
          x1="0"
          y1={VIEW_H * 0.55}
          x2={VIEW_W}
          y2={VIEW_H * 0.55}
          stroke="rgba(255,255,255,0.1)"
          strokeWidth="1"
        />
        <line
          className="fp-wave fp-w3"
          x1="0"
          y1={VIEW_H * 0.65}
          x2={VIEW_W}
          y2={VIEW_H * 0.65}
          stroke="rgba(255,255,255,0.08)"
          strokeWidth="1"
        />

        {/* 水草（根部摇曳） */}
        <path
          className="fp-weed fp-wd1"
          d="M 30 545 Q 35 430 25 340"
          stroke="#2d6a4f"
          strokeWidth="4"
          fill="none"
          strokeLinecap="round"
        />
        <path
          className="fp-weed fp-wd2"
          d="M 45 545 Q 50 450 42 360"
          stroke="#40916c"
          strokeWidth="3"
          fill="none"
          strokeLinecap="round"
        />
        <path
          className="fp-weed fp-wd3"
          d="M 420 545 Q 415 440 425 350"
          stroke="#2d6a4f"
          strokeWidth="4"
          fill="none"
          strokeLinecap="round"
        />
        <path
          className="fp-weed fp-wd4"
          d="M 405 545 Q 400 460 408 370"
          stroke="#40916c"
          strokeWidth="3"
          fill="none"
          strokeLinecap="round"
        />

        {/* 气泡（上浮消散，相位错开） */}
        <circle
          className="fp-bub fp-b1"
          cx="80"
          cy="400"
          r="3"
          fill="rgba(255,255,255,0.3)"
        />
        <circle
          className="fp-bub fp-b2"
          cx="90"
          cy="450"
          r="2"
          fill="rgba(255,255,255,0.25)"
        />
        <circle
          className="fp-bub fp-b3"
          cx="380"
          cy="420"
          r="3"
          fill="rgba(255,255,255,0.3)"
        />
        <circle
          className="fp-bub fp-b4"
          cx="370"
          cy="470"
          r="2"
          fill="rgba(255,255,255,0.25)"
        />

        {/* 鱼群（外层 g 只吃 CSS 洄游动画，内层保持摆位） */}
        {/* 蓝鱼（中景） */}
        <g className="fp-fish fp-f1">
          <g transform="translate(110, 390)">
            <ellipse cx="0" cy="0" rx="18" ry="8" fill="#5a9fd4" />
            <polygon points="-18,0 -28,-6 -28,6" fill="#4a8fc4" />
            <circle cx="8" cy="-2" r="2" fill="#fff" />
            <circle cx="9" cy="-2" r="1" fill="#000" />
          </g>
        </g>
        {/* 橙鱼（近景） */}
        <g className="fp-fish fp-f2">
          <g transform="translate(330, 470)">
            <ellipse cx="0" cy="0" rx="22" ry="10" fill="#e8945a" />
            <polygon points="-22,0 -34,-8 -34,8" fill="#d8844a" />
            <circle cx="10" cy="-3" r="2.5" fill="#fff" />
            <circle cx="11" cy="-3" r="1.2" fill="#000" />
            {/* 条纹 */}
            <line
              x1="-5"
              y1="-8"
              x2="-5"
              y2="8"
              stroke="#c8743a"
              strokeWidth="1.5"
            />
            <line
              x1="0"
              y1="-9"
              x2="0"
              y2="9"
              stroke="#c8743a"
              strokeWidth="1.5"
            />
          </g>
        </g>
        {/* 远景小鱼 */}
        <g className="fp-fish fp-f3">
          <g transform="translate(190, 320)">
            <ellipse
              cx="0"
              cy="0"
              rx="10"
              ry="4"
              fill="#8ab4dd"
              opacity="0.6"
            />
            <polygon points="-10,0 -16,-3 -16,3" fill="#7aa4cd" opacity="0.6" />
          </g>
        </g>
        <g className="fp-fish fp-f4">
          <g transform="translate(290, 300)">
            <ellipse
              cx="0"
              cy="0"
              rx="8"
              ry="3.5"
              fill="#b8a8d8"
              opacity="0.5"
            />
            <polygon
              points="-8,0 -13,-2.5 -13,2.5"
              fill="#a898c8"
              opacity="0.5"
            />
          </g>
        </g>

        {/* 浮漂（钓鱼线 + 浮标） */}
        {inWater && (
          <g>
            {/* 钓鱼线 */}
            <line
              x1={VIEW_W / 2}
              y1={VIEW_H * 0.4}
              x2={VIEW_W / 2}
              y2={VIEW_H * 0.55}
              stroke="#fff"
              strokeWidth="1"
              opacity="0.6"
            />
            {/* 浮漂：待机随波轻浮，咬钩抖动 */}
            <circle
              cx={VIEW_W / 2}
              cy={VIEW_H * 0.55}
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
