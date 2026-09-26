"use client";

import { useState } from "react";
import { TorrentFilterSheet } from "./filter-sheet";
import { useI18n } from "@/i18n/client";

/** 筛选 FAB（M3）：<md 右下角悬浮，唤出筛选 Bottom Sheet。
 *  抬离底 TabBar（+72px）与 safe-area。 */
export function FilterFab() {
  const [open, setOpen] = useState(false);
  const { dict } = useI18n();
  return (
    <div className="md:hidden">
      <button
        type="button"
        className="filter-fab"
        aria-label={dict.torrents2.advanced}
        onClick={() => setOpen(true)}
      >
        <svg
          width="18"
          height="18"
          viewBox="0 0 24 24"
          fill="none"
          stroke="currentColor"
          strokeWidth="2"
          strokeLinecap="round"
        >
          <path d="M4 6h16M7 12h10M10 18h4" />
        </svg>
        {dict.torrents2.advanced}
      </button>
      <TorrentFilterSheet open={open} onClose={() => setOpen(false)} />
    </div>
  );
}
