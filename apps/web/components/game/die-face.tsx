/** 猜大小数字骰（样图⑥的骰面视觉 × 后端 1-100 数域）。
 *  后端掷的是 1-100，双六面骰表达不了（>12 全是两枚 6）——改成
 *  「十位骰 + 个位骰」两枚十面骰拼出 1-100。纯哑件，不参与结算。 */

/** 十面骰点阵：0-9 每个值的点位（viewBox 56×56 内） */
const PIPS_10: Record<number, [number, number][]> = {
  0: [[28, 28]],
  1: [[20, 20], [36, 36]],
  2: [[18, 18], [28, 28], [38, 38]],
  3: [
    [18, 18],
    [38, 18],
    [28, 28],
    [18, 38],
    [38, 38],
  ],
  4: [
    [18, 17],
    [38, 17],
    [28, 28],
    [18, 39],
    [38, 39],
    [28, 17.5],
  ],
  5: [
    [18, 16],
    [38, 16],
    [28, 22],
    [18, 28],
    [38, 28],
    [18, 40],
    [38, 40],
  ],
  6: [
    [18, 15],
    [38, 15],
    [28, 21],
    [18, 28],
    [38, 28],
    [18, 41],
    [38, 41],
    [28, 35],
  ],
  7: [
    [18, 15],
    [38, 15],
    [28, 20],
    [18, 26],
    [38, 26],
    [28, 32],
    [18, 41],
    [38, 41],
  ],
  8: [
    [18, 15],
    [38, 15],
    [28, 20],
    [18, 26],
    [38, 26],
    [28, 32],
    [18, 41],
    [38, 41],
    [28, 42],
  ],
  9: [
    [18, 14],
    [38, 14],
    [28, 19],
    [18, 25],
    [38, 25],
    [28, 30],
    [18, 36],
    [38, 36],
    [28, 42],
    [18, 42.5],
  ],
};

export function DieFace({
  n,
  variant = "six",
  size = 72,
}: {
  n: number;
  /** six=经典六面骰点位；tens/ones=十面骰（1-100 的十位/个位） */
  variant?: "six" | "tens" | "ones";
  size?: number;
}) {
  const pips = PIPS_10[Math.min(9, Math.max(0, n))] ?? PIPS_10[0];
  const label =
    variant === "tens" ? "×10" : variant === "ones" ? "×1" : undefined;
  return (
    <div className="bs-die" style={{ width: size, height: size }}>
      <svg
        width={size}
        height={size}
        viewBox="0 0 56 56"
        role="img"
        aria-label={`${variant}-${n}`}
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
      {label && <span className="bs-die-tag">{label}</span>}
    </div>
  );
}
