"use client";

import { useCallback, useEffect, useState } from "react";
import { api } from "@/lib/api-client";
import { useI18n } from "@/i18n/client";
import { dateLocale, fmt } from "@/i18n/config";
import { PageHeader } from "@/components/ui/page-header";
import { FilterLinks } from "@/components/ui/filter-links";
import { Badge } from "@/components/ui/badge";
import { SkeletonRows } from "@/components/ui/skeleton";
import { EmptyState } from "@/components/ui/empty-state";

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

/** 样图 § achievements 的三态：已达成 / 进行中（带百分比）/ 未开始。
 *  「未开始」这一档字典没给独立键，落 `locked`（未达成）——见交付说明。 */
type AchState = "done" | "doing" | "locked";

function pctOf(a: AchievementRow): number {
  if (a.earned) return 100;
  if (a.metric_value === null) return 0;
  const p = Math.round((a.metric_value / Math.max(a.threshold, 1)) * 100);
  return Math.min(100, Math.max(0, p));
}

function stateOf(a: AchievementRow): AchState {
  if (a.earned) return "done";
  return pctOf(a) > 0 ? "doing" : "locked";
}

/** 卡片两处同形行排版（标题行 / 底部奖励行） */
const CARD_ROW = "flex items-baseline justify-between gap-2";
const CARD_ROW_SUB = "mt-1 flex justify-between text-xs text-sub";

/** 成就墙（GET /me/achievements）：四族卡片，达成由 worker 自动发奖，
 *  所以卡片上只有「已发放」状态，没有「领取」按钮——全站没有领取口。 */
export default function AchievementsPage() {
  const { dict, locale } = useI18n();
  const t = dict.achievements;
  const [rows, setRows] = useState<AchievementRow[] | null>(null);
  const [failed, setFailed] = useState(false);
  const [flt, setFlt] = useState("");

  const load = useCallback(() => {
    api
      .get<AchievementRow[]>("/api/v1/me/achievements")
      .then((d) => {
        setRows(d);
        setFailed(false);
      })
      .catch(() => {
        setRows(null);
        setFailed(true);
      });
  }, []);
  useEffect(load, [load]);

  const list = rows ?? [];
  const count = (s: AchState) => list.filter((a) => stateOf(a) === s).length;
  const done = count("done");
  const points = list
    .filter((a) => a.earned)
    .reduce((n, a) => n + a.reward_sparks, 0);
  const shown = flt ? list.filter((a) => stateOf(a) === flt) : list;
  const families = [...new Set(shown.map((r) => r.family))];
  const familyLabel: Record<string, string> = {
    seed: t.familySeed,
    rescue: t.familyRescue,
    upload: t.familyUpload,
    forum: t.familyForum,
  };

  return (
    <div className="flex flex-col gap-4">
      <PageHeader
        eyebrow={`${dict.my.title} / ${t.title}`}
        title={t.title}
        sub={t.subtitle}
        aside={<Badge tone="brand">{fmt(t.badgePoints, { n: points })}</Badge>}
      />

      <div className="pgstats">
        <div>
          <div className="k">{t.kpiTotal}</div>
          <div className="v">
            {list.length.toLocaleString()}{" "}
            <small>
              {done.toLocaleString()} {t.earned}
            </small>
          </div>
        </div>
        <div>
          <div className="k">{t.kpiDoing}</div>
          <div className="v">{count("doing").toLocaleString()}</div>
        </div>
        <div>
          <div className="k">{t.kpiLocked}</div>
          <div className="v">{count("locked").toLocaleString()}</div>
        </div>
      </div>

      <FilterLinks
        label={t.title}
        value={flt}
        onChange={(v) => setFlt(v ?? "")}
        options={[
          { label: t.tabAll },
          { label: t.tabDone, value: "done" },
          { label: t.tabDoing, value: "doing" },
          { label: t.tabLocked, value: "locked" },
        ]}
      />

      {failed ? (
        <EmptyState
          icon="trophy"
          tone="warn"
          title={dict.readState.failTitle}
          desc={dict.readState.failDesc}
        />
      ) : rows === null ? (
        <SkeletonRows />
      ) : list.length === 0 ? (
        <EmptyState icon="trophy" title={t.empty} />
      ) : shown.length === 0 ? (
        <EmptyState icon="trophy" title={t.emptyFiltered} desc={t.subtitle} />
      ) : (
        families.map((fam) => (
          <section key={fam} className="baozi-panel p-4">
            <h2 className="mb-3 text-base font-bold">
              {familyLabel[fam] ?? t.familyOther}
            </h2>
            <div className="grid gap-3 md:grid-cols-2 xl:grid-cols-3">
              {shown
                .filter((r) => r.family === fam)
                .map((a) => {
                  const pct = pctOf(a);
                  const st = stateOf(a);
                  const stLabel =
                    st === "done"
                      ? t.earned
                      : st === "doing"
                        ? t.kpiDoing
                        : t.locked;
                  return (
                    <article
                      key={a.code}
                      className={`rounded-[var(--r-md)] border p-3 ${
                        a.earned
                          ? "border-sky bg-sky-soft"
                          : "border-line bg-cloud"
                      }`}
                    >
                      <header className={CARD_ROW}>
                        <h3 className="font-bold">{a.name}</h3>
                        <span
                          className={`text-xs font-bold ${
                            a.earned ? "text-sky" : "text-sub"
                          }`}
                        >
                          {stLabel}
                        </span>
                      </header>
                      <p className="mt-1 text-xs text-sub">{a.descr}</p>
                      <div className="mt-2 flex items-center gap-2 text-xs">
                        <progress
                          className="h-1.5 flex-1"
                          max={100}
                          value={pct}
                          aria-label={t.progress}
                        />
                        <span className="num">
                          {(a.metric_value ?? 0).toLocaleString()} /{" "}
                          {a.threshold.toLocaleString()}
                        </span>
                      </div>
                      <div className={CARD_ROW_SUB}>
                        <span>
                          {t.reward} +{a.reward_sparks.toLocaleString()}
                        </span>
                        <span>
                          {a.granted_at
                            ? `${t.granted} ${new Date(
                                a.granted_at,
                              ).toLocaleDateString(dateLocale(locale))}`
                            : st === "doing"
                              ? `${pct}%`
                              : ""}
                        </span>
                      </div>
                    </article>
                  );
                })}
            </div>
          </section>
        ))
      )}
    </div>
  );
}
