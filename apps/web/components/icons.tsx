/**
 * FluxTorrent · TIDE 图标系统（P3）
 *
 * 设计规格（新增图标前必读）：
 *  - 画布 24×24 / 安全区 2px；线性描边 fill=none、stroke=currentColor、默认 1.5px
 *  - linecap=round / linejoin=round —— 与主题「细描边 + hairline」语言一致
 *  - **不内置颜色**：颜色一律继承 currentColor，故浅色/夜间主题自动适配
 *  - 命名走业务语义（`preserve` 保种、`resurrect` 断种复活），不走形状
 *
 * 用法：
 *   <Icon name="seed" />                    // 默认 20
 *   <Icon name="download" size={16} />
 *   <Icon name="spark" className="text-[var(--sky-text)]" />
 *   <Icon name="preserve" title="认领保种" /> // 承载信息时传 title（自动 role=img + <title>）
 *
 * 这是纯展示组件（无 hook / 无事件），**刻意不加 "use client"**：
 * 这样它在 RSC（如 layout.tsx 顶栏与底部 TabBar）里按服务端组件渲染，
 * 在 "use client" 组件（如 theme-toggle）里也能正常被打包使用。
 */
import type { SVGProps } from "react";
import { PATHS } from "./icon-paths";

export type IconName = keyof typeof PATHS;

export interface IconProps extends Omit<SVGProps<SVGSVGElement>, "name"> {
  /** 图标名（见 PATHS）；未注册的名字静默返回 null，不抛错打断页面 */
  name: IconName | string;
  /** 边长 px，默认 20 */
  size?: number;
  /** 描边宽度，默认 1.5 */
  strokeWidth?: number;
  /** 承载信息时传入：渲染 <title> 并以 role="img" 暴露；不传则 aria-hidden */
  title?: string;
}

export function Icon({
  name,
  size = 20,
  strokeWidth = 1.5,
  title,
  className = "",
  ...rest
}: IconProps) {
  const glyph = PATHS[name];
  if (!glyph) return null;
  return (
    <svg
      viewBox="0 0 24 24"
      width={size}
      height={size}
      fill="none"
      stroke="currentColor"
      strokeWidth={strokeWidth}
      strokeLinecap="round"
      strokeLinejoin="round"
      className={className}
      aria-hidden={title ? undefined : true}
      role={title ? "img" : undefined}
      focusable="false"
      {...rest}
    >
      {title ? <title>{title}</title> : null}
      {glyph}
    </svg>
  );
}

/** 已注册图标名（供设计稿对照与单测枚举，防拼错） */
export const ICON_NAMES = Object.keys(PATHS) as IconName[];

export default Icon;
