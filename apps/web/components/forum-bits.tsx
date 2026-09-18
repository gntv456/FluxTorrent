import React from "react";

/** 帖子类型徽标（0115）：normal 不显示；bounty/poll/lottery 用语义色区分。
 *  颜色全部取主题 token（--coral/--mint/--info），明暗主题自动适配，不写死色值。 */
const TYPE_META: Record<string, { emoji: string; color: string }> = {
  bounty: { emoji: "💰", color: "var(--coral)" },
  poll: { emoji: "📊", color: "var(--mint)" },
  lottery: { emoji: "🎁", color: "var(--info)" },
};

export function TypeBadge({
  type,
  label,
  className = "",
}: {
  type?: string | null;
  label?: string;
  className?: string;
}) {
  if (!type || type === "normal") return null;
  const meta = TYPE_META[type];
  if (!meta) return null;
  return (
    <span
      className={`inline-flex items-center gap-0.5 rounded-full border border-line bg-[var(--surface-sunken)] px-1.5 py-0.5 text-[11px] font-bold ${className}`}
      style={{ color: meta.color }}
      title={label ?? type}
    >
      <span aria-hidden="true">{meta.emoji}</span>
      {label ?? type}
    </span>
  );
}
