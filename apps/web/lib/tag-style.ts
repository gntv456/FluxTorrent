import type { CSSProperties } from "react";

/**
 * 标签字典 → 芯片内联样式（唯一口径）。
 *
 * 病形：`tag_dict.color` 的列默认值是白色，它预设「背后一定有底色」。
 * 而站型包导入路径（staff_http/pack_tags.rs）只写 name/kind/scope，底色与字色
 * 全落默认 ⇒ 存进来就是「空底 + 白字」。成员端种子标签条因为有
 * `d.bg_color ? … : undefined` 的守卫躲过了，后台预览与论坛 TagChip 没有，
 * 于是芯片框里连字都看不见（2026-11-02 站长在「标签字典」列表页看到的现象：
 * 官种 / 合集 / 完结 / 官方 的预览是空白方块）。
 *
 * 规矩只有一条：**字色跟着底色走**。没配底色 = 整套配色交回主题令牌，
 * 而不是单独信一个存库的白色。
 */
export interface TagStyleSource {
  bg_color?: string | null;
  color?: string | null;
  font_size?: string | null;
  margin?: string | null;
  padding?: string | null;
  border_radius?: string | null;
}

/** 有底色时返回该标签的配色；没底色返回 null，由调用方走主题缺省。 */
export function tagPaint(tag?: TagStyleSource | null): {
  background: string;
  color: string;
} | null {
  const bg = (tag?.bg_color ?? "").trim();
  if (!bg) return null;
  // 有底色才认存库字色；老数据里 color 可能是空串，回落白字（底色惯例是深色）
  const fg = (tag?.color ?? "").trim();
  // 深底白字是标签芯片惯例，写令牌反而失真
  return { background: bg, color: fg || "#ffffff" }; // design-cov-exempt
}

/**
 * 芯片样式：底色缺省用 `fallbackBg`（论坛用 --surface-sunken，后台预览用
 * 透明 + 虚线框以示意「未配色」），字色在没有底色时一律走 `var(--ink)`。
 */
export function tagChipStyle(
  tag: TagStyleSource,
  fallbackBg = "var(--surface-sunken)",
): CSSProperties {
  const paint = tagPaint(tag);
  return {
    background: paint ? paint.background : fallbackBg,
    color: paint ? paint.color : "var(--ink)",
    fontSize: tag.font_size || undefined,
    margin: tag.margin || undefined,
    padding: tag.padding || undefined,
    borderRadius: tag.border_radius || undefined,
  };
}
