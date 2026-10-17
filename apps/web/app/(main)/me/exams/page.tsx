"use client";

import { Suspense, useCallback, useEffect, useState } from "react";
import { useSearchParams } from "next/navigation";
import { api } from "@/lib/api-client";
import { useI18n } from "@/i18n/client";
import { dateLocale } from "@/i18n/config";
import { formatBytes } from "@/lib/format";
import { PageHeader } from "@/components/ui/page-header";
import { TabPanel, Tabs } from "@/components/ui/tabs";
import { Badge } from "@/components/ui/badge";
import { SkeletonRows } from "@/components/ui/skeleton";
import { EmptyState } from "@/components/ui/empty-state";

interface ExamRow {
  task_id: number;
  name: string;
  subtitle: string | null;
  metric: Record<string, number>;
  kind: string;
  period: string;
  status: number;
  claimed_at: string;
  settled_at: string | null;
  deadline: string | null;
  reward: number;
  penalty: number;
  current: {
    uploaded?: number;
    seed_seconds?: number;
    uploads?: number;
    subtitles?: number;
  };
}

const TABS = ["new", "monthly", "records"] as const;
type TabKey = (typeof TABS)[number];

/** 样图 § exams 三档：新手考核 / 月度考核按 `kind` 分，
 *  第三档「考核记录」是已收尾那批（`status` 非进行中或已有 settled_at），
 *  与前两档交叉——它看的是结论，不是任务来源。
 *  无 `?tab=` 时按已有行挑一档（数据没回来前先落第一档，避免闪一下）。 */
function pickTab(raw: string | null, rows: ExamRow[] | null): TabKey {
  if (raw === "new" || raw === "monthly" || raw === "records") return raw;
  const list = rows ?? [];
  if (rows === null || list.some((e) => e.kind === "onboard")) return "new";
  if (list.some((e) => e.kind === "periodic")) return "monthly";
  return "records";
}

function inTab(e: ExamRow, tab: TabKey): boolean {
  if (tab === "new") return e.kind === "onboard";
  if (tab === "monthly") return e.kind === "periodic";
  return e.status !== 0 || e.settled_at !== null;
}

/** useSearchParams 必须在 Suspense 边界内（预渲染路由要求），
 *  所以本组件由页面壳包一层再渲染。 */
function ExamsPanel() {
  const { dict, locale } = useI18n();
  const t = dict.exams;
  const sp = useSearchParams();
  const [rows, setRows] = useState<ExamRow[] | null>(null);
  // P-314：失败此前写 setRows([]) —— 本 state 有「未知」三态（null），
  // 给 [] 等于把「读不到」冒充成「你没有进行中的考核」（考核是义务信息，
  // 误判成本高）。置 null + 记 failed，空态文案与失败文案分开走。
  const [failed, setFailed] = useState(false);
  // exams 模块守卫：缺键视为开（与网关/导航口径一致）
  const [gated, setGated] = useState(false);

  const load = useCallback(() => {
    api
      .get<ExamRow[]>("/api/v1/me/exams")
      .then(setRows)
      .catch(() => {
        setRows(null);
        setFailed(true);
      });
  }, []);
  useEffect(load, [load]);
  useEffect(() => {
    fetch("/api/v1/site-profile")
      .then((r) => r.json())
      .then(
        (b: { data?: { modules?: Record<string, boolean> } }) => {
          if (b?.data?.modules?.exams === false) setGated(true);
        },
      )
      .catch(() => {});
  }, []);
  if (gated) {
    return (
      <div className="mx-auto max-w-xl px-4 py-16 text-center">
        <p className="text-4xl" aria-hidden>
          🚧
        </p>
        <h1 className="mt-4 text-lg font-semibold">
          {dict.mod.disabledTitle}
        </h1>
        <p className="mt-2 text-sm text-muted">{dict.mod.disabledBody}</p>
      </div>
    );
  }

  const metricLabel: Record<string, string> = {
    upload_delta: t.mUpload,
    download_delta: t.mDownload,
    seed_seconds_delta: t.mSeedSeconds,
    seed_points_delta: t.mSeedPoints,
    uploads: t.mUploads,
    subtitles: t.mSubtitles,
  };

  /** 指标键 → (current) → 展示值。字节键走 formatBytes，秒键折算小时 */
  const renderCurrent = (key: string, cur: ExamRow["current"]): string => {
    if (key === "upload_delta" && cur.uploaded !== undefined)
      return formatBytes(cur.uploaded);
    if (key === "download_delta") return "—"; // 累计口径，结算时判定，不展示增量
    if (key === "seed_seconds_delta" && cur.seed_seconds !== undefined)
      return `${(cur.seed_seconds / 3600).toFixed(1)} ${t.hoursUnit}`;
    if (key === "seed_points_delta" && cur.seed_seconds !== undefined)
      return `${(cur.seed_seconds / 3600).toFixed(0)}`;
    if (key === "uploads" && cur.uploads !== undefined)
      return String(cur.uploads);
    if (key === "subtitles" && cur.subtitles !== undefined)
      return String(cur.subtitles);
    return "—";
  };
  const renderTarget = (key: string, v: number): string => {
    if (key === "upload_delta" || key === "download_delta")
      return formatBytes(v);
    if (key === "seed_seconds_delta")
      return `${(v / 3600).toFixed(0)} ${t.hoursUnit}`;
    return v.toLocaleString();
  };

  const all = rows ?? [];
  const tab = pickTab(sp.get("tab"), rows);
  const shown = all.filter((e) => inTab(e, tab));
  const href = (k: TabKey) => `/me/exams?tab=${k}`;
  const at = (s: string) => new Date(s).toLocaleString(dateLocale(locale));
  const isNewcomer = shown.some((e) => e.kind === "onboard");

  return (
    <div className="flex flex-col gap-4">
      <PageHeader
        eyebrow={`${dict.my.title} / ${t.title}`}
        title={t.title}
        sub={t.subtitle}
        aside={
          isNewcomer ? (
            <span className="sticker">{t.badgeNewcomer}</span>
          ) : null
        }
      />

      <div className="pgstats">
        <div>
          <div className="k">{t.kpiTotal}</div>
          <div className="v">{all.length.toLocaleString()}</div>
        </div>
        <div>
          <div className="k">{t.kpiPassed}</div>
          <div className="v">
            {all.filter((e) => e.status === 1).length.toLocaleString()}
          </div>
        </div>
        <div>
          <div className="k">{t.kpiPending}</div>
          <div className="v">
            {all.filter((e) => e.status === 0).length.toLocaleString()}
          </div>
        </div>
      </div>

      <Tabs
        idPrefix="exam"
        label={t.title}
        value={tab}
        hrefs={[href("new"), href("monthly"), href("records")]}
        items={[
          { key: "new", label: t.tabNew },
          { key: "monthly", label: t.tabMonthly },
          { key: "records", label: t.tabRecords },
        ]}
      />

      <TabPanel idPrefix="exam" tabKey={tab}>
        {failed ? (
          <EmptyState
            icon="exam"
            tone="warn"
            title={dict.readState.failTitle}
            desc={dict.readState.failDesc}
          />
        ) : rows === null ? (
          <SkeletonRows />
        ) : all.length === 0 ? (
          <EmptyState icon="exam" title={t.empty} />
        ) : shown.length === 0 ? (
          <EmptyState icon="exam" title={t.emptyFiltered} desc={t.subtitle} />
        ) : (
          <div className="baozi-panel">
            <div className="baozi-wide-table-scroll">
              <table className="nexus-table">
                <thead>
                  <tr>
                    <td className="colhead">{t.colName}</td>
                    <td className="colhead">{t.colStatus}</td>
                    <td className="colhead">{t.colProgress}</td>
                    <td className="colhead">{t.colClaimedAt}</td>
                    <td className="colhead">{t.colSettled}</td>
                    <td className="colhead">{t.colDeadline}</td>
                    <td className="colhead">{t.colReward}</td>
                  </tr>
                </thead>
                <tbody>
                  {shown.map((e) => (
                    <tr key={e.task_id}>
                      <td>
                        <span className="font-bold">{e.name}</span>
                        <div className="text-xs text-sub">
                          {e.kind === "onboard"
                            ? t.kindOnboard
                            : t.kindPeriodic}
                          {e.period === "monthly"
                            ? ` · ${t.periodMonthly}`
                            : e.period === "quarterly"
                              ? ` · ${t.periodQuarterly}`
                              : ""}
                          {e.subtitle ? ` · ${e.subtitle}` : ""}
                        </div>
                      </td>
                      <td>
                        <Badge
                          tone={
                            e.status === 1
                              ? "success"
                              : e.status === 0
                                ? "warn"
                                : "danger"
                          }
                        >
                          {e.status === 0
                            ? t.stOngoing
                            : e.status === 1
                              ? t.stDone
                              : t.stFailed}
                        </Badge>
                      </td>
                      <td className="text-xs">
                        {Object.entries(e.metric).map(([k, v]) => (
                          <div key={k}>
                            {metricLabel[k] ?? k}：
                            {renderCurrent(k, e.current)} /{" "}
                            {renderTarget(k, v)}
                          </div>
                        ))}
                      </td>
                      <td className="num text-xs text-sub">
                        {at(e.claimed_at)}
                      </td>
                      <td className="num text-xs text-sub">
                        {e.settled_at ? at(e.settled_at) : "—"}
                      </td>
                      <td className="num text-xs text-sub">
                        {e.deadline ? at(e.deadline) : "—"}
                      </td>
                      <td className="num font-bold">
                        +{e.reward.toLocaleString()}
                      </td>
                    </tr>
                  ))}
                </tbody>
              </table>
            </div>
          </div>
        )}
      </TabPanel>

      <div className="baozi-panel">
        <div className="baozi-panel__head">
          <h2>{t.ruleTitle}</h2>
        </div>
        <p className="p-4 text-xs leading-relaxed text-sub">{t.ruleBody}</p>
      </div>
    </div>
  );
}

export default function MyExamsPage() {
  return (
    <Suspense fallback={<SkeletonRows />}>
      <ExamsPanel />
    </Suspense>
  );
}
