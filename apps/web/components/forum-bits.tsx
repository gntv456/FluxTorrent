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

/** 论坛标签样式（0123）：词表与样式列全部来自 tag_dict（0063 带样式的通用标签字典，
 *  站长在 admin-tagdict 后台改色即全站生效）。样式列可能为空串（历史数据），空值回落主题默认。 */
export interface TagChipData {
  id: number;
  name: string;
  kind?: string;
  bg_color?: string;
  color?: string;
  font_size?: string;
  margin?: string;
  padding?: string;
  border_radius?: string;
}

export function TagChip({
  tag,
  className = "",
}: {
  tag: TagChipData;
  className?: string;
}) {
  return (
    <span
      className={`inline-block font-bold leading-normal ${className}`}
      style={{
        background: tag.bg_color || "var(--surface-sunken)",
        color: tag.color || "var(--ink)",
        fontSize: tag.font_size || "11px",
        margin: tag.margin || undefined,
        padding: tag.padding || "1px 6px",
        borderRadius: tag.border_radius || "9999px",
      }}
    >
      {tag.name}
    </span>
  );
}
