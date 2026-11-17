/** Wiki 目录页分组卡片（RSC 服务端直出——SEO/无 JS 可见；0495）。
 *
 *  检索交互由 <WikiSearch>（client）按 data-wiki-card/data-title 做
 *  DOM 过滤（渐进增强），这里只出结构：检索框 + 空态占位 + 分组卡。
 *  data-wiki-section 挂在分组卡上，过滤命中空时整卡隐藏。 */

import { WikiSearch } from "@/components/wiki-search";
import { PANEL_BRAND, WIKI_CARD_LINK } from "@/lib/ui-classes";

export interface WikiIndexEntry {
  id: number;
  slug: string;
  title: string;
  section: string;
  updated_at: string;
}

export interface WikiSectionDef {
  key: string;
  title: string;
  desc: string;
}

export function WikiIndex({
  entries,
  sections,
  searchPlaceholder,
  searchEmpty,
  updatedAtLabel,
  pagesUnit,
}: {
  entries: WikiIndexEntry[];
  sections: WikiSectionDef[];
  searchPlaceholder: string;
  searchEmpty: string;
  updatedAtLabel: string;
  pagesUnit: (n: number) => string;
}) {
  return (
    <div className="flex flex-col gap-4">
      <div className="max-w-xl">
        <WikiSearch placeholder={searchPlaceholder} />
      </div>
      <p
        id="wiki-filter-empty"
        style={{ display: "none" }}
        className="text-sm text-sub"
      >
        {searchEmpty}
      </p>
      {sections.map((s) => {
        const items = entries.filter((e) => e.section === s.key);
        if (items.length === 0) return null;
        return (
          <section
            key={s.key}
            data-wiki-section={s.key}
            className={PANEL_BRAND}
          >
            <header className="flex flex-wrap items-baseline gap-2">
              <h2 className="font-display text-lg font-bold text-ink">
                {s.title}
              </h2>
              <span className="text-xs text-sub">
                {pagesUnit(items.length)}
              </span>
            </header>
            <p className="mt-0.5 text-xs text-sub">{s.desc}</p>
            <ul className="mt-3 grid gap-2 sm:grid-cols-2 xl:grid-cols-3">
              {items.map((p) => (
                <li key={p.id}>
                  <a
                    href={`/wiki/${p.slug}`}
                    data-wiki-card={p.slug}
                    data-title={p.title}
                    data-slug={p.slug}
                    className={WIKI_CARD_LINK}
                  >
                    <span className="text-sm font-medium text-link">
                      {p.title}
                    </span>
                    <span className="mt-1 text-xs text-sub">
                      {updatedAtLabel}: {p.updated_at.slice(0, 10)}
                    </span>
                  </a>
                </li>
              ))}
            </ul>
          </section>
        );
      })}
    </div>
  );
}
