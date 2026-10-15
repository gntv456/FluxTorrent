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
