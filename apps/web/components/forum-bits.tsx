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

// TagChipData 已收编 @fluxtorrent/domain-types；re-export 保持既有导入路径
export type { TagChipData } from "@fluxtorrent/domain-types";
import type { TagChipData as _TCD } from "@fluxtorrent/domain-types";

export function TagChip({
  tag,
  className = "",
}: {
  tag: _TCD;
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

/** 置顶/精华的**实心五角星**（样图集 § tplForum/tplForumBoard 的
 *  starPin/starEss）。app 的 Icon 是线性描边（fill=none），观感与样图
 *  的实心星不同，故照抄样图那条 path，用 currentColor 由外部上色：
 *  置顶 text-coral、精华 text-sun。 */
export function StarMark({ tone }: { tone: "coral" | "sun" }) {
  return (
    <svg
      width="13"
      height="13"
      viewBox="0 0 24 24"
      fill="currentColor"
      aria-hidden="true"
      className={`shrink-0 ${tone === "coral" ? "text-coral" : "text-sun"}`}
    >
      <path
        d="M12 2l2.4 6.9L21 9l-5.3 4.2 1.7 6.8L12 16.6
           6.6 20l1.7-6.8L3 9l6.6-.1z"
      />
    </svg>
  );
}

/** 话题标记文字徽章（样图 badge--pin/sun）：置顶 / 精华。
 *  样式（底色/字色/圆角）在 forums-home.css 的 .fh-badge 系。 */
export function MarkBadge({
  kind,
  label,
}: {
  kind: "pin" | "ess";
  label: string;
}) {
  return <span className={`fh-badge fh-badge--${kind}`}>{label}</span>;
}
