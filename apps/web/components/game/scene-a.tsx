/**
 * 玩法机台画面（波·游戏化视觉）：每款玩法一张「机台屏幕」SVG——
 * 画的是**这台机器在做什么**（骰盅待开、转盘待转、鱼群游过），
 * 不是装饰纹样。大厅卡面与玩法页舞台共用同一张 scene：
 * 卡面 = 机台外观，玩法页 = 走进机台（口径见 .zcode/skills/no-ai-design-slop
 * ——元素必须解释「这是什么游戏」，换个 logo 认不出的就是 slop）。
 *
 * 零位图资源依赖（仓库口径：emoji/SVG）；色走 TIDE/甜梦令牌；
 * 纯静态组件（reduced-motion 天然安全，动效由 CSS class 承担）。
 * 300 行门禁：拆 scene-a.tsx（即时开奖 5 款）/ scene-b.tsx（养成 4 款）。
 */

type P = { className?: string };

/** 猜大小机台：三骰在骰盅里待开，大/小双灯柱 */
export function SceneBigsmall({ className }: P) {
  return (
    <svg viewBox="0 0 220 110" className={className} aria-hidden>
      <rect x="0" y="0" width="220" height="110" fill="url(#bs-felt)" />
      <defs>
        <radialGradient id="bs-felt" cx="50%" cy="30%" r="90%">
          <stop offset="0%" stopColor="var(--mint)" stopOpacity="0.4" />
          <stop offset="100%" stopColor="var(--sky)" stopOpacity="0.15" />
        </radialGradient>
      </defs>
      {/* 大/小灯柱 */}
      <rect
        x="30"
        y="30"
        width="26"
        height="56"
        rx="6"
        fill="var(--coral)"
        opacity="0.75"
      />
      <text
        x="43"
        y="64"
        textAnchor="middle"
        fontSize="13"
        fontWeight="900"
        fill="#fff"
      >
        小
      </text>
      <rect
        x="164"
        y="30"
        width="26"
        height="56"
        rx="6"
        fill="var(--sky-deep)"
        opacity="0.8"
      />
      <text
        x="177"
        y="64"
        textAnchor="middle"
        fontSize="13"
        fontWeight="900"
        fill="#fff"
      >
        大
      </text>
      {/* 骰盅碗 */}
      <path
        d="M70 42 A40 40 0 0 0 150 42 Z"
        fill="var(--surface-card)"
        stroke="var(--sweet-gold)"
        strokeWidth="2"
      />
      <path
        d="M70 42 A40 26 0 0 1 150 42"
        fill="var(--surface-raised)"
        stroke="var(--sweet-gold)"
        strokeWidth="1.5"
      />
      {/* 碗里三骰（微斜） */}
      {[
        [92, 52, -12],
        [110, 50, 8],
        [128, 52, -4],
      ].map(([x, y, r], i) => (
        <g key={i} transform={`rotate(${r} ${x} ${y})`}>
          <rect
            x={x - 8}
            y={y - 8}
            width="16"
            height="16"
            rx="3.5"
            fill="#fff"
            stroke="var(--sweet-gold)"
          />
          <circle cx={x - 3} cy={y - 3} r="1.6" fill="var(--ink)" />
          <circle cx={x + 3} cy={y + 3} r="1.6" fill="var(--ink)" />
        </g>
      ))}
      {/* 台面金线 */}
      <line
        x1="8"
        y1="100"
        x2="212"
        y2="100"
        stroke="var(--sweet-gold)"
        strokeWidth="1"
        opacity="0.5"
        strokeDasharray="6 5"
      />
    </svg>
  );
}

/** 刮刮乐机台：信封斜放，涂层格半开，硬币正在刮 */
export function SceneScratch({ className }: P) {
  return (
    <svg viewBox="0 0 220 110" className={className} aria-hidden>
      <rect width="220" height="110" fill="url(#sc-sky)" />
      <defs>
        <linearGradient id="sc-sky" x1="0" y1="0" x2="0" y2="1">
          <stop offset="0%" stopColor="var(--coral)" stopOpacity="0.35" />
          <stop offset="100%" stopColor="var(--sun)" stopOpacity="0.2" />
        </linearGradient>
      </defs>
      <g transform="rotate(-6 110 55)">
        <rect
          x="58"
          y="20"
          width="104"
          height="70"
          rx="8"
          fill="var(--surface-card)"
          stroke="var(--sweet-gold)"
          strokeWidth="1.5"
        />
        <path
          d="M58 28 L110 56 L162 28"
          fill="none"
          stroke="var(--sweet-gold)"
          strokeWidth="1.5"
        />
        {[0, 1, 2].map((i) => (
          <g key={i}>
            <rect
              x={72 + i * 28}
              y={44}
              width="22"
              height="18"
              rx="3"
              fill="var(--sweet-gold)"
              opacity={i === 1 ? 0.25 : 0.85}
            />
            {i === 1 && (
              <text
                x={83 + i * 28}
                y={57}
                textAnchor="middle"
                fontSize="10"
                fontWeight="900"
                fill="var(--coral)"
              >
                2x
              </text>
            )}
          </g>
        ))}
        {[0, 1, 2].map((i) => (
          <rect
            key={`b${i}`}
            x={72 + i * 28}
            y={66}
            width="22"
            height="14"
            rx="3"
            fill="var(--sweet-gold)"
            opacity="0.85"
          />
        ))}
      </g>
      {/* 硬币 */}
      <circle
        cx="176"
        cy="74"
        r="10"
        fill="var(--sun)"
        stroke="var(--sweet-gold)"
        strokeWidth="1.5"
      />
      <text
        x="176"
        y="78"
        textAnchor="middle"
        fontSize="10"
        fontWeight="900"
        fill="var(--ink)"
      >
        ¥
      </text>
    </svg>
  );
}

/** 九宫格机台：3×3 灯阵两列已亮，跑灯光标停在倒数第二格 */
export function SceneJgg({ className }: P) {
  const lit = [0, 1, 3, 4, 6, 7];
  const cur = 5;
  return (
    <svg viewBox="0 0 220 110" className={className} aria-hidden>
      <rect width="220" height="110" fill="url(#jg-night)" />
      <defs>
        <radialGradient id="jg-night" cx="50%" cy="40%" r="85%">
          <stop offset="0%" stopColor="var(--candy)" stopOpacity="0.45" />
          <stop offset="100%" stopColor="var(--sky)" stopOpacity="0.18" />
        </radialGradient>
      </defs>
      {Array.from({ length: 9 }, (_, i) => {
        const x = 64 + (i % 3) * 34;
        const y = 16 + Math.floor(i / 3) * 28;
        const on = lit.includes(i);
        return (
          <rect
            key={i}
            x={x}
            y={y}
            width="28"
            height="22"
            rx="5"
            fill={
              on
                ? "var(--sun)"
                : i === cur
                  ? "var(--coral)"
                  : "var(--surface-card)"
            }
            opacity={on ? 0.9 : i === cur ? 0.95 : 0.6}
            stroke={i === cur ? "var(--sun)" : "var(--border-deep)"}
            strokeWidth={i === cur ? 2 : 1}
          />
        );
      })}
      {/* 顶灯排 */}
      {[70, 100, 130, 160].map((x, i) => (
        <circle
          key={`t${i}`}
          cx={x}
          cy={10}
          r="3"
          fill="var(--sun)"
          opacity={i % 2 ? 0.4 : 0.95}
        />
      ))}
    </svg>
  );
}
