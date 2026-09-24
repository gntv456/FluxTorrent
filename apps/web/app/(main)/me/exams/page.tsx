"use client";

import { useCallback, useEffect, useState } from "react";
import { api } from "@/lib/api-client";
import { useI18n } from "@/i18n/client";
import { dateLocale } from "@/i18n/config";
import { formatBytes } from "@/lib/format";

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

/** 我的考核（0093）：进行中/已完成/失败的考核 + 当前值 vs 目标进度。
 *  客户端页无法直接用 RSC 版 requireModule——开关关闭时渲染统一空态（二审 G2-2）。 */
export default function MyExamsPage() {
  const { dict, locale } = useI18n();
  const t = dict.exams;
  const [rows, setRows] = useState<ExamRow[] | null>(null);
  // exams 模块守卫：缺键视为开（与网关/导航口径一致）
  const [gated, setGated] = useState(false);

  const load = useCallback(() => {
    api
      .get<ExamRow[]>("/api/v1/me/exams")
      .then(setRows)
      .catch(() => setRows([]));
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

  return (
    <div className="flex flex-col gap-4">
      <div className="flex flex-wrap items-baseline gap-2">
        <h1 className="font-display text-2xl">{t.title}</h1>
        <span className="text-sm text-sub">{t.subtitle}</span>
      </div>
      <div className="baozi-wide-table-scroll">
        <table className="nexus-table">
          <tbody>
            <tr>
              <td className="colhead">{t.colName}</td>
              <td className="colhead">{t.colStatus}</td>
              <td className="colhead">{t.colProgress}</td>
              <td className="colhead">{t.colClaimedAt}</td>
              <td className="colhead">{t.colDeadline}</td>
              <td className="colhead">{t.colReward}</td>
            </tr>
            {(rows ?? []).map((e) => (
              <tr key={e.task_id}>
                <td>
                  <span className="font-bold">{e.name}</span>
                  <div className="text-xs text-sub">
                    {e.kind === "onboard" ? t.kindOnboard : t.kindPeriodic}
                    {e.period === "monthly"
                      ? ` · ${t.periodMonthly}`
                      : e.period === "quarterly"
                        ? ` · ${t.periodQuarterly}`
                        : ""}
                    {e.subtitle ? ` · ${e.subtitle}` : ""}
                  </div>
                </td>
                <td>
                  {e.status === 0
                    ? t.stOngoing
                    : e.status === 1
                      ? t.stDone
                      : t.stFailed}
                </td>
                <td className="text-xs">
                  {Object.entries(e.metric).map(([k, v]) => (
                    <div key={k}>
                      {metricLabel[k] ?? k}：{renderCurrent(k, e.current)} /{" "}
                      {renderTarget(k, v)}
                    </div>
                  ))}
                </td>
                <td className="text-xs text-sub">
                  {new Date(e.claimed_at).toLocaleString(dateLocale(locale))}
                </td>
                <td className="text-xs text-sub">
                  {e.deadline
                    ? new Date(e.deadline).toLocaleString(dateLocale(locale))
                    : "—"}
                </td>
                <td className="num font-bold">+{e.reward.toLocaleString()}</td>
              </tr>
            ))}
            {rows?.length === 0 && (
              <tr>
                <td colSpan={6} className="py-6 text-center text-sub">
                  {t.empty}
                </td>
              </tr>
            )}
          </tbody>
        </table>
      </div>
    </div>
  );
}
