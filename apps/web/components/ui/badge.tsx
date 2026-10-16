import type { HTMLAttributes, ReactNode } from "react";

/** 徽章原语（S2 模板化 · 2026-10-16）
 *
 *  统一「状态 / 属性标签」的形态，替代此前散落的 `.sticker` + Tailwind
 *  色直写（`sticker bg-mint/30 text-ink` 等 18 处）——调用方只表达语义
 *  （这是「成功 / 警告 / 危险」），不再表达颜色（DS-PLAN-01 §10.1 宪章 2）。
 *
 *  纯展示组件（无 hook / 无事件），刻意不加 "use client"：RSC 与
 *  Client 组件里都能直接用（同 Icon / SkeletonRows 口径）。
 *  余下 HTML 属性（title / data-* / aria-*）原样透传到 <span>。
 *
 *  用法：
 *    <Badge tone="success">免费</Badge>
 *    <Badge tone="brand" variant="solid">官方</Badge>
 *    <Badge tone="warn" title="解释">置顶</Badge>
 */
export function Badge({
  tone = "neutral",
  variant = "soft",
  size = "md",
  className,
  children,
  ...rest
}: {
  /** 语义色档（对齐 base.css 的 --tone-* 令牌，主题切换自动生效） */
  tone?: "brand" | "neutral" | "success" | "warn" | "danger";
  /** soft=浅底深字（默认，状态标签）/ solid=实底反白（醒目标记） */
  variant?: "soft" | "solid";
  /** 尺寸三档（sm 10px / md 11px 默认 / lg 12px） */
  size?: "sm" | "md" | "lg";
  children: ReactNode;
} & Omit<HTMLAttributes<HTMLSpanElement>, "children">) {
  const cls = [
    "ui-badge",
    `ui-badge--${tone}`,
    `ui-badge--${variant}`,
    `ui-badge--${size}`,
    className ?? "",
  ]
    .filter(Boolean)
    .join(" ");
  return (
    <span className={cls} {...rest}>
      {children}
    </span>
  );
}
