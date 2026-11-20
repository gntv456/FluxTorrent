"use client";

import { useI18n } from "@/i18n/client";

export interface HeatCell {
  month: string;
  days: number;
}

/** 签到热力足迹（0425 后端 / 0498 前端收口）：近 12 个月逐月签到天数条。
 *  数据源 GET /attendance 的 heatmap 字段（本批补齐的独立下发口）。
 *  纯 CSS 色阶（tone 令牌四级），无图表库依赖；移动端横滚兜底。
 *  embedded=签到页重排版（2026-11-20）嵌入日历卡尾的形态：去 brand-panel
 *  卡面（外壳由宿主卡提供），标题也交给宿主（避免双标题）。 */
export function AttendanceHeatmap({
  cells,
  embedded,
}: {
  cells: HeatCell[];
  embedded?: boolean;
}) {
  const { dict } = useI18n();
  const t = dict.checkin.heat;
  if (!cells || cells.length === 0) return null;
  const max = Math.max(...cells.map((c) => c.days), 1);
  if (embedded) {
    return (
      <div className="flex items-end gap-1.5 overflow-x-auto pb-1">
        {cells.map((c) => {
          const level =
            c.days === 0
              ? 0
              : c.days <= max * 0.25
                ? 1
                : c.days <= max * 0.5
                  ? 2
                  : 3;
          return (
            <div
              key={c.month}
              className="flex flex-col items-center gap-1 shrink-0"
              title={`${c.month} · ${c.days}`}
            >
              <div
                className={`heat-cell heat-cell--l${level}`}
                style={{ height: `${14 + (c.days / max) * 30}px` }}
              />
              <span className="text-[10px] opacity-60">
                {c.month.slice(5)}
              </span>
            </div>
          );
        })}
      </div>
    );
  }
  return (
    <section className="brand-panel p-4" aria-label={t.title}>
      <div className="flex items-baseline justify-between mb-2">
        <h3 className="text-sm font-medium">{t.title}</h3>
        <span className="text-xs opacity-60">{t.hint}</span>
      </div>
      <div className="flex items-end gap-1.5 overflow-x-auto pb-1">
        {cells.map((c) => {
          const level =
            c.days === 0
              ? 0
              : c.days <= max * 0.25
                ? 1
                : c.days <= max * 0.5
                  ? 2
                  : 3;
          return (
            <div
              key={c.month}
              className="flex flex-col items-center gap-1 shrink-0"
              title={`${c.month} · ${c.days}`}
            >
              <div
                className={`heat-cell heat-cell--l${level}`}
                style={{ height: `${20 + (c.days / max) * 44}px` }}
              />
              <span className="text-[10px] opacity-60">
                {c.month.slice(5)}
              </span>
            </div>
          );
        })}
      </div>
    </section>
  );
}
