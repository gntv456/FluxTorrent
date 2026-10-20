"use client";

import Link from "next/link";
import { useEffect, useRef, useState } from "react";
import type { WorkListItem } from "@fluxtorrent/domain-types";
import { PosterImage } from "@/components/poster-image";
import { workTitle } from "@/components/work-chips";

/** 相关推荐只排一行：按栅格实际列数裁剪，多出来的不渲染
 *  （不撑出第二行，也不留看不见的 Tab 焦点）。 */
export function WorkRelatedGrid({
  items,
  cats,
}: {
  items: WorkListItem[];
  cats: Record<number, string>;
}) {
  const ref = useRef<HTMLDivElement>(null);
  const [cap, setCap] = useState(items.length);

  useEffect(() => {
    const el = ref.current;
    if (!el) return;
    const fit = () => {
      const tracks = getComputedStyle(el).gridTemplateColumns;
      const n = tracks.split(/\s+/).filter(Boolean).length;
      if (n > 0) setCap(Math.min(n, items.length));
    };
    fit();
    const ro = new ResizeObserver(fit);
    ro.observe(el);
    return () => ro.disconnect();
  }, [items.length]);

  return (
    <div className="wk-related" ref={ref}>
      {items.slice(0, cap).map((r) => {
        const name = workTitle(r.name);
        return (
          <Link key={r.key} href={`/works/${r.key}`} className="wk-rel">
            <span className="wk-rel-p">
              <PosterImage
                src={r.poster ?? (r.meta?.source ? r.meta.poster : null)}
                alt=""
                size="lg"
                className="h-full w-full object-cover"
                fallback={<span className="wk-rel-fb">{name}</span>}
              />
            </span>
            <span className="wk-rel-info">
              <span className="wk-rel-name">{name}</span>
              <span className="wk-rel-y">
                {[r.year, cats[r.category_id]]
                  .filter(Boolean)
                  .join(" · ")}
              </span>
            </span>
          </Link>
        );
      })}
    </div>
  );
}
