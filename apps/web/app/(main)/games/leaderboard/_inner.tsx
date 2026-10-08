"use client";

import Link from "next/link";
import { useState } from "react";
import { PANEL_LG_COL } from "@/lib/ui-classes";
import { useI18n } from "@/i18n/client";
import { dateLocale } from "@/i18n/config";

export interface BoardData {
  week: string;
  boards: {
    lucky: { who: string; v: number }[];
    fishing: { who: string; v: number }[];
    farmer: { who: string; v: number }[];
    grinder: { who: string; v: number }[];
  };
}

type BoardKey = keyof BoardData["boards"];

/** 周榜（只读）：欧皇 / 渔王 / 农夫 / 劳模。不派奖、不写账。 */
export default function LeaderboardPage({
  initial,
}: {
  initial: BoardData | null;
}) {
  const { dict, locale, currency } = useI18n();
  const tl = dict.games.lb;
  const [tab, setTab] = useState<BoardKey>("lucky");
  const data = initial;
  const rows = data?.boards[tab] ?? [];
  const magic = tab === "lucky" || tab === "fishing";
  const fmt = (v: number) =>
    magic
      ? `${v.toLocaleString(dateLocale(locale))} ${currency}`
      : `${v.toLocaleString(dateLocale(locale))} ${tl.times}`;
  const pill =
    "rounded-full border px-3 py-1 text-xs font-bold transition-colors";
  const tabs: { key: BoardKey; label: string; tip: string }[] = [
    { key: "lucky", label: tl.lucky, tip: tl.luckyTip },
    { key: "fishing", label: tl.fishing, tip: tl.fishingTip },
    { key: "farmer", label: tl.farmer, tip: tl.farmerTip },
    { key: "grinder", label: tl.grinder, tip: tl.grinderTip },
  ];

  return (
    <div className="flex flex-col gap-4">
      <section className="lb-hero">
        <div className="pg-eyebrow">Arcade · Weekly</div>
        <h1 className="font-display">{tl.title}</h1>
        <p className="text-xs text-sub">
          {tl.sub}
          {data ? ` · ${data.week}` : ""}
        </p>
      </section>

      <div className="flex flex-wrap gap-1.5">
        {tabs.map((b) => (
          <button
            key={b.key}
            type="button"
            onClick={() => setTab(b.key)}
            title={b.tip}
            className={`${pill} ${
              tab === b.key
                ? "border-sky bg-sky text-white"
                : "border-line bg-[var(--surface-card)] text-sub hover:text-ink"
            }`}
            aria-pressed={tab === b.key}
          >
            {b.label}
          </button>
        ))}
      </div>

      <div className={PANEL_LG_COL}>
        <div className="lb-table" role="table">
          <div className="lb-row lb-head" role="row">
            <span>{tl.rank}</span>
            <span>{tl.player}</span>
            <span className="num">{tl.value}</span>
          </div>
          {rows.map((r, i) => (
            <div
              key={r.who}
              className={`lb-row${i < 3 ? ` top${i + 1}` : ""}`}
              role="row"
            >
              <span className="num lb-rank" aria-hidden>
                {i === 0 ? "🥇" : i === 1 ? "🥈" : i === 2 ? "🥉" : i + 1}
              </span>
              <span className="lb-who">{r.who}</span>
              <span className="num">{fmt(r.v)}</span>
            </div>
          ))}
          {rows.length === 0 && (
            <p className="arc-note">{tl.empty}</p>
          )}
        </div>
        <p className="text-[11px] text-sub">{tl.note}</p>
      </div>

      <div className="flex flex-wrap gap-x-5 gap-y-2">
        <Link
          href="/games"
          className="text-xs font-bold text-[var(--sky-deep)]"
        >
          {dict.games.back} →
        </Link>
      </div>
    </div>
  );
}
