"use client";

import type { ReactNode } from "react";
import { Badge } from "@/components/ui/badge";
import { useI18n } from "@/i18n/client";
import type { AttendanceRules } from "@/components/checkin-rules";

/** 签到页主动作卡（重排版 2026-11-20）：把旧版「统计条 + 规则文字卡 +
 *  热力条」三块平铺压成一个 hero——主动作（大按钮）+ 今日可得（真实预估）
 *  + 周期进度 + 里程碑链条，页面第一眼就是「签到能拿多少」。
 *
 *  今日可得预估与后端 checkin_reward 完全同构（economy_checkin.rs）：
 *  base（首次 first / 日常 streak）→ 周期内位置命中里程碑 → min(daily_cap)。
 *  参数全部来自 GET /attendance 的 rules 透出（站长改配置即跟随），
 *  无任何前端臆造数字（no-ai-design-slop：每个数字都有数据源）。 */

/** 与后端同构的奖励预估：streak_after = 连签数（签到后）。 */
export function previewReward(
  streakAfter: number,
  isFirstEver: boolean,
  rules: AttendanceRules,
): { base: number; bonus: number; total: number } {
  const base = isFirstEver ? rules.first : rules.streak;
  const cycle =
    rules.cycle_days ??
    (rules.milestones ?? []).reduce((m, [d]) => Math.max(m, d), 30);
  const pos =
    cycle > 0 ? (((Math.max(streakAfter, 1) - 1) % cycle) + 1) : streakAfter;
  const bonus = (rules.milestones ?? []).find(([d]) => d === pos)?.[1] ?? 0;
  return { base, bonus, total: Math.min(base + bonus, rules.daily_cap) };
}

export function CheckinHero({
  checkedToday,
  streak,
  totalDays,
  makeupCards,
  rules,
  busy,
  msg,
  onCheckin,
  checkinLabel,
  busyLabel,
  attendedLabel,
  resub,
}: {
  checkedToday: boolean;
  streak: number;
  totalDays: number;
  makeupCards: number;
  rules?: AttendanceRules;
  busy: boolean;
  msg: string | null;
  onCheckin: () => void;
  checkinLabel: string;
  busyLabel: string;
  attendedLabel: string;
  /** 补签入口（弹层件，含持有数徽标）——与签到主按钮并排（2026-11-22
   *  体验反馈：原页尾右下角太不显眼） */
  resub?: ReactNode;
}) {
  const { dict, currency } = useI18n();
  const t = dict.home2;
  // 周期视角（后端 rules 透出；旧后端缺字段时前端按 milestones 推导）
  const cycle =
    rules?.cycle_days ??
    (rules?.milestones ?? []).reduce((m, [d]) => Math.max(m, d), 30);
  const pos =
    cycle > 0
      ? (((Math.max(streak, 1) - 1) % cycle) + 1)
      : Math.max(streak, 1);
  // 今日可得：未签时预估 = checkin(streak+1)；已签时展示今日实得
  // （recent 末行 reward 由父级传入？不——直接按同构公式对 streak+1 预估，
  // 已签态换成「今日已签」徽标，不再展示预估避免与实得混淆）。
  const gain =
    rules && !checkedToday
      ? previewReward(streak + 1, totalDays === 0, rules)
      : null;
  const nextMs =
    rules?.next_milestone_in_cycle ??
    (rules?.milestones ?? [])
      .map(([d]) => d)
      .filter((d) => d > pos)
      .sort((a, b) => a - b)[0] ??
    null;
  const nextBonus =
    (rules?.milestones ?? []).find(([d]) => d === nextMs)?.[1] ?? 0;
  const pct = cycle > 0 ? Math.min(100, Math.round((pos / cycle) * 100)) : 0;
  return (
    <section className="ck-hero" aria-label={t.attendanceTitle}>
      <div className="ck-hero__main">
        <div className="ck-hero__gain">
          <span className="ck-hero__gain-label">
            {checkedToday ? attendedLabel : t.heroTodayTitle}
          </span>
          {gain ? (
            <strong className="ck-hero__gain-num">
              +{gain.total}
              <small> {currency}</small>
            </strong>
          ) : (
            <strong className="ck-hero__gain-num is-done">✓</strong>
          )}
          {gain && gain.bonus > 0 ? (
            <Badge tone="brand" size="sm">
              {t.rulesMilestone
                .replace("{day}", String(pos))
                .replace("{bonus}", String(gain.bonus))
                .replace("{total}", String(gain.total))}
            </Badge>
          ) : null}
        </div>
        <div className="ck-hero__act">
          <div className="ck-hero__actions">
            {!checkedToday ? (
              <button
                type="button"
                className="ck-hero__btn"
                onClick={onCheckin}
                disabled={busy}
              >
                {busy ? busyLabel : checkinLabel}
              </button>
            ) : (
              <span className="ck-hero__done">{attendedLabel}</span>
            )}
            {resub}
          </div>
          <p className="ck-hero__meta">
            {t.heroDay.replace("{pos}", String(pos))}
            {" · "}
            {t.heroTotalLine
              .replace("{streak}", String(streak))
              .replace("{total}", String(totalDays))}
          </p>
        </div>
      </div>
      {/* 周期进度条：本周期第 pos / cycle 天——里程碑链条的刻度即它 */}
      {rules ? (
        <div className="ck-hero__cycle">
          <div
            className="ck-hero__track"
            role="progressbar"
            aria-valuenow={pos}
            aria-valuemin={1}
            aria-valuemax={cycle}
          >
            <i style={{ width: `${pct}%` }} />
            {(rules.milestones ?? []).map(([d, bonus]) => (
              <em
                key={d}
                className={d <= pos ? "is-hit" : undefined}
                title={`${d} · +${bonus}`}
                style={{
                  left: `${Math.min(100, (d / cycle) * 100)}%`,
                }}
              />
            ))}
          </div>
          <p className="ck-hero__cycle-note">
            {nextMs
              ? t.heroNextMs
                  .replace("{day}", String(nextMs))
                  .replace("{bonus}", String(nextBonus))
              : t.heroMsNow.replace("{pos}", String(pos))}
            {" · "}
            {t.heroCardNote.replace("{cards}", String(makeupCards))}
          </p>
        </div>
      ) : null}
      {msg ? <p className="attendance-msg">{msg}</p> : null}
    </section>
  );
}
