"use client";

import { useEffect, useId } from "react";
import Link from "next/link";
import { useI18n } from "@/i18n/client";
import { uxMark } from "@/lib/ux-metrics";
import { Badge } from "@/components/ui/badge";

export interface CategoryLink {
  id: number;
  name: string;
}

/** 最新资源空态的主视觉：按站点顶级分类引导发现，而不是摆一排假海报。
 *  分类来自首页 RSC 已取的站点档案，链接走既有筛选参数，不新增请求。 */
export function HomeCategoryGuide({
  categories,
  compact = false,
}: {
  categories: CategoryLink[];
  compact?: boolean;
}) {
  const { dict } = useI18n();
  const uid = useId();
  useEffect(() => {
    if (categories.length > 0) uxMark("empty_state", "home-latest");
  }, [categories.length]);

  if (categories.length === 0) return null;
  const label = dict.uxOpt.exploreCategories;
  return (
    <section
      className={compact ? "ux-catguide ux-catguide--inline" : "ux-catguide"}
      aria-labelledby={uid + "-title"}
    >
      <h2 id={uid + "-title"} className="ux-catguide__title">
        {label}
      </h2>
      <nav className="ux-catguide__grid" aria-label={label}>
        {categories.slice(0, 8).map((c) => (
          <Link
            key={c.id}
            href={"/torrents?category_id=" + c.id}
            className="ux-catguide__item"
            onClick={() => uxMark("first_torrent_open", "home-latest-category")}
          >
            <Badge tone="neutral" size="sm">{c.name}</Badge>
            <span className="ux-catguide__arrow" aria-hidden="true">
              ›
            </span>
          </Link>
        ))}
      </nav>
    </section>
  );
}
