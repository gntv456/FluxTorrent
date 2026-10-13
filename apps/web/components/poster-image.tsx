"use client";

import { useState } from "react";
import { Icon } from "@/components/icons";

/** 海报/封面统一渲染件（海报逻辑收敛：此前 5 处各自 <img> + 手写回退）。
 *  统一行为：2:3 海报口径宽高比 + object-cover + --surface-sunken 占位底、
 *  loading=lazy / decoding=async / referrerPolicy=no-referrer（外链图床
 *  不带 referrer，规避按来源防盗链的误杀）、无图或加载失败（onError）
 *  → 渲染 fallback（缺省为占位底 + 唱片图标），挂图不显示破图。
 *
 *  刻意整体 client 化（而非 CSS 回退保持 server）：onError 只能在 client
 *  附加，而 CSS 无法检测 <img> 加载失败、藏不住破图；组件唯一状态就是
 *  一个 broken 布尔，props 全可序列化，server 消费方走「RSC 渲染 client
 *  子组件」标准模式（fallback 传 ReactNode 跨边界），SSR 首帧照常直出。 */

/** 海报尺寸档（宽度；高度一律由 2:3 宽高比推导）：
 *  sm=表格行内封面 46px；md=卡片封面 79px；lg=宽度交给外层容器。 */
export type PosterSize = "sm" | "md" | "lg";

const SIZE_CLS: Record<PosterSize, string> = {
  sm: "w-[46px]",
  md: "w-[79px]",
  lg: "w-full",
};

/** 圆角档（映射 --r-* 令牌）；默认 none——多数调用点的圆角已由外层卡片
 *  overflow-hidden 或既有 CSS（.torrents-cover / .td-head__img）控制。 */
const ROUNDED_CLS: Record<"none" | "sm" | "md", string> = {
  none: "",
  sm: "rounded-[var(--r-sm)]",
  md: "rounded-[var(--r-md)]",
};

export function PosterImage({
  src,
  alt,
  size = "md",
  className,
  rounded = "none",
  sizes,
  fallback,
}: {
  /** 海报外链；null/undefined 时直接走 fallback（不渲染 <img>） */
  src?: string | null;
  alt: string;
  size?: PosterSize;
  className?: string;
  rounded?: keyof typeof ROUNDED_CLS;
  /** 原生 img sizes 布局提示（卡片/海报墙按需透传） */
  sizes?: string;
  /** 无图/挂图时的整块替换渲染（如分类色块 + 唱片图标）；缺省用内置占位 */
  fallback?: React.ReactNode;
}) {
  const [broken, setBroken] = useState(false);
  const cls = [
    SIZE_CLS[size],
    "aspect-[2/3] object-cover bg-[var(--surface-sunken)]",
    ROUNDED_CLS[rounded],
    className,
  ]
    .filter(Boolean)
    .join(" ");

  if (src && !broken) {
    return (
      // eslint-disable-next-line @next/next/no-img-element
      <img
        src={src}
        alt={alt}
        loading="lazy"
        decoding="async"
        referrerPolicy="no-referrer"
        sizes={sizes}
        onError={() => setBroken(true)}
        className={cls}
      />
    );
  }
  if (fallback !== undefined) return <>{fallback}</>;
  return (
    <span
      className={`${cls} flex items-center justify-center`}
      aria-hidden
    >
      <Icon name="disc" size={22} className="opacity-60" />
    </span>
  );
}
