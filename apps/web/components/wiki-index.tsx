"use client";

/** Wiki 目录页客户端壳：检索框 + 分组卡片列表（即时过滤）。
 *
 *  全量目录由 RSC 页拿好传进来（服务端渲染首屏，SEO/无 JS 都有内容），
 *  这里只做交互层：击键过滤。分组顺序与组文案由 RSC 侧排好，
 *  过滤命中空时显示 searchEmpty 提示（不隐藏分组标题——用户改词时
 *  卡片框架还在，视觉不闪）。 */
import { useState } from "react";

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
  const [q, setQ] = useState("");
  const needle = q.trim().toLowerCase();
  const hit = (e: WikiIndexEntry) =>
    !needle ||
    e.title.toLowerCase().includes(needle) ||
    e.slug.toLowerCase().includes(needle);
  const visible = entries.filter(hit);
  const total = visible.length;

  return (
    <div className="flex flex-col gap-4">
      <div className="max-w-xl">
        <WikiSearch placeholder={searchPlaceholder} onFilter={setQ} />
      </div>
      {total === 0 && (
        <div className={PANEL_BRAND}>
          <p className="text-sm text-sub">{searchEmpty}</p>
        </div>
      )}
      {sections.map((s) => {
        const items = visible.filter((e) => e.section === s.key);
        if (items.length === 0) return null;
        return (
          <section key={s.key} className={PANEL_BRAND}>
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
