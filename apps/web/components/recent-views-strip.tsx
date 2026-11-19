"use client";

import { useEffect, useState } from "react";
import Link from "next/link";
import { useI18n } from "@/i18n/client";
import {
  clearRecentViews,
  readRecentViews,
  type RecentView,
} from "@/lib/recent-views";
import { uxMark } from "@/lib/ux-metrics";

/** 最近浏览续接条：让回访用户从「上次看到哪」继续，而不是重新找。
 *  只在有记录时渲染；清空即时生效，不打扰无记录用户。 */
export function RecentViewsStrip() {
  const { dict } = useI18n();
  const [items, setItems] = useState<RecentView[] | null>(null);

  useEffect(() => {
    setItems(readRecentViews());
  }, []);

  if (!items || items.length === 0) return null;

  return (
    <section className="ux-recent" aria-label={dict.uxOpt.recentViewsTitle}>
      <header className="ux-recent__head">
        <h2 className="ux-recent__title">{dict.uxOpt.recentViewsTitle}</h2>
        <button
          type="button"
          className="ux-recent__clear"
          onClick={() => {
            clearRecentViews();
            setItems([]);
          }}
        >
          {dict.uxOpt.recentViewsClear}
        </button>
      </header>
      <ul className="ux-recent__list">
        {items.map((v) => (
          <li key={v.id}>
            <Link
              href={"/torrent/" + v.id}
              className="ux-recent__item"
              title={v.name}
              data-ux-event="first_torrent_open"
              data-ux-label="recent-views"
              onClick={() => uxMark("first_torrent_open", "recent-views")}
            >
              <span className="ux-recent__name">{v.name}</span>
              <span aria-hidden="true">›</span>
            </Link>
          </li>
        ))}
      </ul>
    </section>
  );
}
