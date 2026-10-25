"use client";

/**
 * 自动审种统计条（0400 A11，策划案 §3.2）：设置页「自动审种」分区顶部的
 * 实时示例——「按当前配置，今天自动通过 X / 拒绝 Y / 暂缓 Z」。
 * 数据源 = /admin/auto-review/stats（管理面批 2 已备）；读失败静默隐藏
 * （统计条是辅助信息，不能挡设置页可用性）。
 */

import { useEffect, useState } from "react";
import { api } from "@/lib/api-client";
import { useI18n } from "@/i18n/client";

type Stats = {
  today: { pass: number; deny: number; postpone: number };
  pending_human: number;
  shadow: { samples: number; agree_pct: number; ready: boolean };
  /** 切执法提示文案（0402 设置键，站点可编辑） */
  switch_hint?: string;
};

export function AutoReviewStatBar() {
  const { dict } = useI18n();
  // autoReview 段已入字典（三语齐全），直接用强类型键
  const ar = dict.admin.autoReview;
  const [stats, setStats] = useState<Stats | null>(null);

  useEffect(() => {
    let alive = true;
    api
      .get<Stats>("/admin/auto-review/stats")
      .then((v) => {
        if (alive) setStats(v);
      })
      .catch(() => {
        /* 静默：无 staff 权或端点异常都不挡设置页 */
      });
    return () => {
      alive = false;
    };
  }, []);

  if (!stats) return null;
  const hint = (stats.switch_hint ?? "").trim();
  return (
    <p
      className="rounded-[var(--r-md)] border border-sky bg-sky-soft px-3 py-2
        text-xs text-ink"
      data-testid="auto-review-stat-bar"
    >
      {ar.statBar
        .replace("{pass}", String(stats.today.pass))
        .replace("{deny}", String(stats.today.deny))
        .replace("{postpone}", String(stats.today.postpone))
        .replace("{pending}", String(stats.pending_human))
        .replace("{agree}", String(stats.shadow.agree_pct))
        .replace("{samples}", String(stats.shadow.samples))}
      {stats.shadow.ready
        ? ` · ${ar.ready}`
        : ""}
      {/* 切执法提示（0402）：站点可编辑的文案，空则不占行 */}
      {hint ? (
        <span className="mt-1 block text-ink-2">{hint}</span>
      ) : null}
    </p>
  );
}
