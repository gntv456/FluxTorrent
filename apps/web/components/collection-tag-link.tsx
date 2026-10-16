import { Badge } from "@/components/ui/badge";

/** 收藏标签链接（2026-10-16 S2 模板化）
 *
 *  原实现是这段逐字重复两遍的写法：
 *    <a href={`/collections/${id}`}
 *       className={`sticker ${kind === "series"
 *         ? "bg-indigo text-white" : "bg-sun text-ink"}`}>
 *  ——链接语义与配色挤在一个 className 里，色类直写（明暗主题不跟随），
 *  且详情页与 _parts/torrent-detail-collections 各抄了一份。
 *
 *  现在分工明确：<a> 只管跳转（保持原生 a，两处都是 server 组件），
 *  视觉交给 Badge 原语，tone/variant 与 collections 页内的同款标签一致。
 *  容器 .td-collections__list 是 flex+gap，不依赖 sticker 的盒子样式。
 */
export function CollectionTagLink({
  href,
  name,
  series,
}: {
  /** 跳转地址，形如 /collections/12 */
  href: string;
  /** 合集名（调用方给原文，本地化在上游） */
  name: string;
  /** true=系列（brand 实底）；false=合集（warn 浅底） */
  series: boolean;
}) {
  return (
    <a href={href} className="inline-flex">
      <Badge
        tone={series ? "brand" : "warn"}
        variant={series ? "solid" : "soft"}
      >
        {name}
      </Badge>
    </a>
  );
}
