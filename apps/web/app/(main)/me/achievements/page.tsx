"use client";

import { useCallback, useEffect, useState } from "react";
import { api } from "@/lib/api-client";
import { useI18n } from "@/i18n/client";
import { dateLocale } from "@/i18n/config";

interface AchievementRow {
  code: string;
  family: string;
  name: string;
  descr: string;
  threshold: number;
  reward_sparks: number;
  earned: boolean;
  granted_at: string | null;
  metric_value: number | null;
}

/** 成就墙（GET /me/achievements）：四族卡片，已达成高亮 + 进度 */
export default function AchievementsPage() {
  const { dict, locale } = useI18n();
  const t = dict.achievements;
  const [rows, setRows] = useState<AchievementRow[] | null>(null);

  const load = useCallback(() => {
    api.get<AchievementRow[]>("/api/v1/me/achievements").then(setRows).catch(() => setRows([]));
  }, []);
  useEffect(load, [load]);

  const familyLabel: Record<string, string> = {
    seed: t.familySeed,
    rescue: t.familyRescue,
    upload: t.familyUpload,
    forum: t.familyForum,
  };
  const families = [...new Set((rows ?? []).map((r) => r.family))];

  return (
    <div className="flex flex-col gap-4">
      <div className="flex flex-wrap items-baseline gap-2">
        <h1 className="font-display text-2xl">{t.title}</h1>
        <span className="text-sm text-sub">{t.subtitle}</span>
      </div>
      {families.map((fam) => (
        <section key={fam} className="baozi-panel p-4">
          <h2 className="mb-3 text-base font-bold">{familyLabel[fam] ?? t.familyOther}</h2>
          <div className="grid gap-3 md:grid-cols-2 xl:grid-cols-3">
            {(rows ?? [])
              .filter((r) => r.family === fam)
              .map((a) => {
                const pct = a.earned
                  ? 100
                  : a.metric_value !== null
                    ? Math.min(100, Math.round((a.metric_value / Math.max(a.threshold, 1)) * 100))
                    : 0;
                return (
                  <article
                    key={a.code}
                    className={`rounded-[var(--r-md)] border p-3 ${
                      a.earned ? "border-sky bg-sky-soft" : "border-line bg-cloud"
                    }`}
                  >
                    <header className="flex items-baseline justify-between gap-2">
                      <h3 className="font-bold">{a.name}</h3>
                      <span className={`text-xs font-bold ${a.earned ? "text-sky" : "text-sub"}`}>
                        {a.earned ? t.earned : t.locked}
                      </span>
                    </header>
                    <p className="mt-1 text-xs text-sub">{a.descr}</p>
                    <div className="mt-2 flex items-center gap-2 text-xs">
                      <progress className="h-1.5 flex-1" max={100} value={pct} />
                      <span className="num">
                        {(a.metric_value ?? 0).toLocaleString()} / {a.threshold.toLocaleString()}
                      </span>
                    </div>
                    <div className="mt-1 flex justify-between text-xs text-sub">
                      <span>{t.reward} +{a.reward_sparks.toLocaleString()}</span>
                      <span>
                        {a.granted_at ? `${t.grantedAt} ${new Date(a.granted_at).toLocaleDateString(dateLocale(locale))}` : ""}
                      </span>
                    </div>
                  </article>
                );
              })}
          </div>
        </section>
      ))}
      {rows !== null && rows.length === 0 && (
        <p className="py-6 text-center text-sub">{t.empty}</p>
      )}
    </div>
  );
}
