"use client";

import { useI18n } from "@/i18n/client";
import { dateLocale } from "@/i18n/config";

/** 任务系统辅助件（从 components/task-board.tsx 按域拆出）：
 *  TaskRow/TaskOverview 数据契约 + 进度/字节/档名工具 +
 *  08 我的任务记录子面板。主组件 TaskBoard 与数据装载留在原文件。 */

export interface TaskRow {
  id: number;
  name: string;
  metric: {
    upload_delta?: number;
    download_delta?: number;
    seed_points_delta?: number;
    uploads?: number;
    subtitles?: number;
    [k: string]: unknown;
  };
  reward: number;
  penalty: number;
  claim_limit: number | null;
  claimed: number;
  starts_at: string;
  ends_at: string;
  tier?: string;
  subtitle?: string;
  fee?: number;
  duration_days?: number;
  quota_total?: number;
  claimed_by_me?: boolean;
}

export interface TaskOverview {
  shop: {
    name: string;
    span: string;
    require_tier: string;
    require_count: number;
    cost: number;
    stock: number;
  }[];
  feed: { user: string; task: string; at: string }[];
  stats: {
    ongoing: number;
    done: number;
    failed: number;
    tiers: { tier: string | null; total: number; done: number; pct: number }[];
  };
  my_records: {
    task_id: number;
    name: string;
    status: number;
    claimed_at: string;
    settled_at: string | null;
    deadline?: string | null;
    reward?: number;
    metric?: Record<string, number>;
    current?: {
      uploaded?: number;
      seed_seconds?: number;
      uploads?: number;
      subtitles?: number;
    };
  }[];
}

/** 我的任务区进度单元格：metric 键 → 现值/目标（口径与结算一致；秒键折算小时） */
function progressCells(
  metric: Record<string, number> | undefined,
  current:
    | {
        uploaded?: number;
        seed_seconds?: number;
        uploads?: number;
        subtitles?: number;
      }
    | undefined,
  fmtBytes: (n: number) => string,
): string {
  if (!metric || !current) return "—";
  return Object.entries(metric)
    .map(([k, v]) => {
      if (k === "upload_delta")
        return `${fmtBytes(current.uploaded ?? 0)} / ${fmtBytes(v)}`;
      if (k === "download_delta") return `${fmtBytes(v)}`;
      if (k === "seed_seconds_delta" || k === "seed_points_delta")
        return `${((current.seed_seconds ?? 0) / 3600).toFixed(1)} / ${(v / 3600).toFixed(0)}h`;
      if (k === "uploads") return `${current.uploads ?? 0} / ${v}`;
      if (k === "subtitles") return `${current.subtitles ?? 0} / ${v}`;
      return `${v}`;
    })
    .join(" · ");
}

export function fmtBytes(n: number): string {
  const units = ["B", "KB", "MB", "GB", "TB", "PB"];
  let v = n;
  let i = 0;
  while (v >= 1024 && i < units.length - 1) {
    v /= 1024;
    i += 1;
  }
  return `${v >= 100 ? v.toFixed(0) : v.toFixed(2)} ${units[i]}`;
}

/** 五档等级 key → 中文档名（与 DB subtitle 对应；NP 站档名即英文，这里展示中文更友好） */
const TIER_LABELS: Record<string, string> = {
  master: "骨灰",
  ultimate: "走火入魔",
  extreme: "烧糊涂",
  veteran: "高烧",
  insane: "中烧",
};
export function tierLabel(tier: string | null | undefined): string {
  if (!tier) return "";
  return TIER_LABELS[tier.toLowerCase()] ?? tier;
}

/** 06 最新动态 + 07 任务统计子面板（数据由 TaskBoard 的 ov 传入） */
export function TaskDashboard({ ov }: { ov: TaskOverview | null }) {
  const { dict, locale } = useI18n();
  const t = dict.tasks2;
  return (
    <div className="task-dashboard-grid">
      <section className="task-panel">
        <div className="task-section-heading">
          <div>
            <span>06</span>
            <h2>{t.feedTitle}</h2>
          </div>
        </div>
        <div className="task-feed">
          {(ov?.feed ?? []).map((f, i) => (
            <div key={i}>
              <p>
                <strong>{f.user}</strong> {t.feedAction} {f.task}
              </p>
              <time>
                {new Date(f.at).toLocaleDateString(dateLocale(locale), {
                  month: "2-digit",
                  day: "2-digit",
                })}{" "}
                {new Date(f.at).toLocaleTimeString(dateLocale(locale), {
                  hour: "2-digit",
                  minute: "2-digit",
                })}
              </time>
            </div>
          ))}
          {(ov?.feed ?? []).length === 0 && (
            <p className="text-sub">{t.feedEmpty}</p>
          )}
        </div>
      </section>
      <section className="task-panel">
        <div className="task-section-heading">
          <div>
            <span>07</span>
            <h2>{t.statsTitle}</h2>
          </div>
        </div>
        <div className="task-stats-summary">
          <div>
            <strong className="num">{ov?.stats.ongoing ?? 0}</strong>
            <span>{t.stOngoing}</span>
          </div>
          <div>
            <strong className="num">{ov?.stats.done ?? 0}</strong>
            <span>{t.stDone}</span>
          </div>
          <div>
            <strong className="num">{ov?.stats.failed ?? 0}</strong>
            <span>{t.stFailed}</span>
          </div>
        </div>
        <div className="task-stats-bars">
          {(ov?.stats.tiers ?? []).map((s) => (
            <div key={s.tier ?? "?"}>
              <span>{tierLabel(s.tier)}</span>
              <progress max={100} value={s.pct} />
              <b className="num">{s.pct.toFixed(1)}%</b>
            </div>
          ))}
        </div>
      </section>
    </div>
  );
}

/** 08 我的任务记录子面板：领取/结算时间 + 进度列（数据由 TaskBoard 的 ov 传入） */
export function TaskHistory({ ov }: { ov: TaskOverview | null }) {
  const { dict, locale } = useI18n();
  const t = dict.tasks2;
  return (
    <section className="task-panel task-history">
      <div className="task-section-heading">
        <div>
          <span>08</span>
          <h2>{t.historyTitle}</h2>
        </div>
      </div>
      <div className="baozi-wide-table-scroll">
        <table className="nexus-table">
          <tbody>
            <tr>
              <td className="colhead">{t.hTask}</td>
              <td className="colhead">{t.hStatus}</td>
              <td className="colhead">{dict.exams.colProgress}</td>
              <td className="colhead">{t.hClaimedAt}</td>
              <td className="colhead">{t.hSettledAt}</td>
            </tr>
            {(ov?.my_records ?? []).map((r, i) => (
              <tr key={i}>
                <td>{r.name}</td>
                <td>
                  {r.status === 0
                    ? t.stOngoing
                    : r.status === 1
                      ? t.stDone
                      : t.stFailed}
                </td>
                <td className="text-xs">
                  {progressCells(r.metric, r.current, fmtBytes)}
                </td>
                <td>
                  {new Date(r.claimed_at).toLocaleString(dateLocale(locale))}
                </td>
                <td>
                  {r.settled_at
                    ? new Date(r.settled_at).toLocaleString(dateLocale(locale))
                    : "—"}
                </td>
              </tr>
            ))}
            {(ov?.my_records ?? []).length === 0 && (
              <tr>
                <td colSpan={5} className="py-6 text-center text-sub">
                  {t.historyEmpty}
                </td>
              </tr>
            )}
          </tbody>
        </table>
      </div>
    </section>
  );
}
