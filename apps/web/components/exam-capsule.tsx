"use client";

import { useEffect, useState } from "react";
import Link from "next/link";
import { api } from "@/lib/api-client";
import { useI18n } from "@/i18n/client";

/** 考核进度顶栏胶囊（0427，同类站 UI 对标采纳③）。
 *
 *  新手考核（onboard/periodic 任务）进行中时在顶栏常驻一枚胶囊：
 *  「考核 62% · 剩 12 天」，点击进 /jixiao 详情。多指标取**最落后一项**
 *  的达成率——考核按项判定（settle：metric 里出现的键全部满足），
 *  最短板才是真实进度；只显示最高项会制造「快达标」错觉。
 *
 *  数据源 GET /me/exams（ops_http/exams.rs，P-527 五源现值）。
 *  目标解析与 worker task_jobs/settle.rs 判据逐键同源：
 *    upload_delta/download_delta 字节；seed_seconds_delta 秒；
 *    seed_points_delta 小时（×3600 折秒）；uploads/subtitles 个数。
 *  无进行中考核 / 未登录 / 接口失败 → 整枚胶囊不渲染（顶栏零噪音）。 */

type ExamRow = {
  task_id: number;
  name: string;
  metric: Record<string, number> | null;
  kind: string;
  status: string;
  deadline: string | null;
  current: Record<string, number>;
};

/** 单指标达成率（0-1）；与 settle 判据同键同折算。 */
function metricRatio(
  key: string,
  target: number,
  cur: Record<string, number>,
): number | null {
  if (!Number.isFinite(target) || target <= 0) return null;
  const v = cur[key] ?? 0;
  return Math.max(0, Math.min(1, v / target));
}

function ratiosOf(row: ExamRow): number[] {
  const m = row.metric ?? {};
  const c = row.current ?? {};
  const out: number[] = [];
  if (m.upload_delta != null)
    out.push(metricRatio("uploaded", m.upload_delta, c) ?? 1);
  if (m.download_delta != null)
    out.push(metricRatio("downloaded", m.download_delta, c) ?? 1);
  if (m.seed_seconds_delta != null)
    out.push(metricRatio("seed_seconds", m.seed_seconds_delta, c) ?? 1);
  // seed_points_delta 单位=做种小时，current.seed_seconds 是秒
  if (m.seed_points_delta != null)
    out.push(
      metricRatio("seed_seconds", m.seed_points_delta * 3600, c) ?? 1,
    );
  if (m.uploads != null) out.push(metricRatio("uploads", m.uploads, c) ?? 1);
  if (m.subtitles != null)
    out.push(metricRatio("subtitles", m.subtitles, c) ?? 1);
  return out;
}

export function ExamCapsule() {
  const { dict } = useI18n();
  const [exam, setExam] = useState<{
    pct: number;
    daysLeft: number | null;
    name: string;
  } | null>(null);

  useEffect(() => {
    let alive = true;
    api
      .get<ExamRow[]>("/api/v1/me/exams")
      .then((rows) => {
        if (!alive) return;
        const active = rows.filter(
          (r) =>
            (r.kind === "onboard" || r.kind === "periodic") &&
            r.status === "active",
        );
        if (!active.length) return setExam(null);
        let worst: { pct: number; row: ExamRow } | null = null;
        for (const r of active) {
          const ratios = ratiosOf(r);
          const pct = ratios.length
            ? Math.round(Math.min(...ratios) * 100)
            : 100;
          if (!worst || pct < worst.pct) worst = { pct, row: r };
        }
        if (!worst) return setExam(null);
        const daysLeft = worst.row.deadline
          ? Math.max(
              0,
              Math.ceil(
                (new Date(worst.row.deadline).getTime() - Date.now()) /
                  86400000,
              ),
            )
          : null;
        setExam({ pct: worst.pct, daysLeft, name: worst.row.name });
      })
      .catch(() => {
        if (alive) setExam(null);
      });
    return () => {
      alive = false;
    };
  }, []);

  if (!exam) return null;
  // YemaPT 口径的环形进度（SVG 圆弧）：百分比一眼可读，颜色随进度走
  // （<50% 橙 · <100% 蓝 · 达成绿）。 circumference=2πr, r=8。
  const R = 8;
  const C = 2 * Math.PI * R;
  const dash = (Math.min(100, Math.max(0, exam.pct)) / 100) * C;
  const ringColor =
    exam.pct >= 100 ? "var(--success)" : exam.pct >= 50
      ? "var(--sky)" : "var(--sun)";
  return (
    <Link
      href="/me/exams"
      title={`${exam.name} · ${dict.exams.capsuleTip}`}
      className="exam-capsule inline-flex items-center gap-1.5 rounded-full
        border border-[var(--sun)] px-2.5 py-1 text-[11.5px] font-semibold
        text-[var(--gold-deep)] hover:bg-[var(--sun)]/10"
    >
      <svg width="20" height="20" viewBox="0 0 20 20" aria-hidden="true">
        <circle cx="10" cy="10" r={R} fill="none"
                stroke="var(--brand-line-soft)" strokeWidth="3" />
        <circle cx="10" cy="10" r={R} fill="none"
                stroke={ringColor} strokeWidth="3"
                strokeDasharray={`${dash} ${C}`}
                strokeLinecap="round"
                transform="rotate(-90 10 10)" />
        <text x="10" y="13" textAnchor="middle"
              fontSize="8" fontWeight="700" fill="currentColor">
          {exam.pct}
        </text>
      </svg>
      <span>{dict.exams.capsule}</span>
      {exam.daysLeft !== null && (
        <span className="num opacity-80">
          {dict.exams.capsuleDays.replace("{d}", String(exam.daysLeft))}
        </span>
      )}
    </Link>
  );
}
