"use client";

/** 连胜徽章（趣味性批）：计数 + 三档升温（3 连金 / 5 连炽 / 8 连烈）。
 *  静态哑件：文案由调用方传（i18n），档位自己按 n 判。 */
export function StreakPill({
  n,
  label,
}: {
  /** 当前连胜局数（>=3 才渲染） */
  n: number;
  /** 文案模板 {n} 会被替换成计数 */
  label: string;
}) {
  if (n < 3) return null;
  const tier = n >= 8 ? "blaze" : n >= 5 ? "hot" : "";
  const fire = n >= 8 ? "🔥🔥🔥" : n >= 5 ? "🔥🔥" : "🔥";
  return (
    <span className={`streak-pill ${tier}`.trimEnd()}>
      <span aria-hidden>{fire}</span>
      {label.replace("{n}", String(n))}
    </span>
  );
}
