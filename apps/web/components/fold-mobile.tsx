"use client";

import { useIsCompact } from "@/lib/hooks/use-media";
import { Fold } from "@/components/torrent-detail-parts";

/** 移动端默认收起、桌面恒开的折叠分区（M5.8 详情页纵向降噪）：
 *  SSR 首帧按桌面口径展开（use-media 的 SSR 安全纪律：宁可首帧多渲染
 *  一版桌面，不冒 hydration markup 不一致的风险），<768 水合后收起——
 *  与数据表卡片化同范式。桌面态与 <details open> 无异，点击折叠照常。 */
export function FoldMobile({
  title,
  count,
  children,
}: {
  title: string;
  count?: number;
  children: React.ReactNode;
}) {
  const compact = useIsCompact();
  return (
    <Fold title={title} count={count} open={!compact}>
      {children}
    </Fold>
  );
}

/** 头部内嵌轻量折叠（M5.9 社交/答谢收纳）：与 FoldMobile 的差别是
 *  桌面直接原样渲染 children（不加 summary 行、无卡片 chrome），
 *  仅移动端包 <details> 收起——用于头部里没有既有标题行的小区块，
 *  避免桌面凭空多出一条折叠标题破坏既有排版。 */
export function FoldMobileBare({
  title,
  children,
}: {
  title: string;
  children: React.ReactNode;
}) {
  const compact = useIsCompact();
  if (!compact) return <>{children}</>;
  return (
    <details className="td-fold-inline">
      <summary>
        <span>{title}</span>
        <span className="td-fold__chev" aria-hidden />
      </summary>
      <div className="td-fold-inline__body">{children}</div>
    </details>
  );
}
