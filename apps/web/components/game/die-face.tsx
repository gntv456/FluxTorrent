/** 猜大小骰面（样图⑥）：白底圆角骰 + 深蓝点，1-6 点位图。
 *  纯哑件：点位表写死属于展示层，不参与任何结算。 */
const PIPS: Record<number, [number, number][]> = {
  1: [[28, 28]],
  2: [
    [19, 19],
    [37, 37],
  ],
  3: [
    [18, 18],
    [28, 28],
    [38, 38],
  ],
  4: [
    [19, 19],
    [37, 19],
    [19, 37],
    [37, 37],
  ],
  5: [
    [19, 17],
    [37, 17],
    [28, 28],
    [19, 39],
    [37, 39],
  ],
  6: [
    [19, 17],
    [37, 17],
    [19, 28],
    [37, 28],
    [19, 39],
    [37, 39],
  ],
};

export function DieFace({
  n,
  size = 72,
  roll = false,
}: {
  n: number;
  size?: number;
  /** 开奖滚动动画（纯视觉） */
  roll?: boolean;
}) {
  const pips = PIPS[Math.min(6, Math.max(1, n))] ?? PIPS[1];
  return (
    <div
      className={`bs-die${roll ? " rolling" : ""}`}
      style={{ width: size, height: size }}
    >
      <svg
        width={size}
        height={size}
        viewBox="0 0 56 56"
        role="img"
        aria-label={`die-${n}`}
      >
        <rect
          x="3"
          y="3"
          width="50"
          height="50"
          rx="11"
          fill="#fff"
          stroke="#d4af7a"
          strokeWidth="2"
        />
        {pips.map(([cx, cy], i) => (
          <circle key={i} cx={cx} cy={cy} r="3.6" fill="#2c3e5c" />
        ))}
      </svg>
    </div>
  );
}
