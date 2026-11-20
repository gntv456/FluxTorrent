"use client";

import { useI18n } from "@/i18n/client";

/** 签到规则数据（GET /attendance 的 rules 透出，2026-11-10 周期制）。 */
export interface AttendanceRules {
  first: number;
  streak: number;
  daily_cap: number;
  milestones: [number, number][];
  /** 周期制（2026-11-10）：满 cycle_days 次日归 1 重计，加送循环发。
   *  cycle_pos=当前周期内位置；next_milestone_in_cycle=下一档在周期内的
   *  天数（null=本周期已过最大档，下个周期第 10 天再来）。 */
  cycle_days?: number;
  cycle_pos?: number;
  next_milestone_in_cycle?: number | null;
}

/** 规则卡（重排版 2026-11-20）：文字列表改里程碑链条——每档一个节点
 *  （天数 + 加送额 + 当日合计），已过档实心、下一档高亮脉冲，一眼看到
 *  「再签几天能多拿多少」。兜底文案（首次/封顶/断签/周期）收进卡底注脚。 */
export function CheckinRules({
  rules,
  streakNow,
}: {
  rules: AttendanceRules;
  streakNow: number;
}) {
  const { dict, currency } = useI18n();
  const t = dict.home2;
  // 周期口径：后端给了 cycle_pos/next 就直接用；旧后端（缺字段）回落
  // 绝对天数口径，保证灰度期两边都能渲染。
  const cycle = rules.cycle_days;
  const pos =
    cycle && cycle > 0
      ? ((Math.max(streakNow, 1) - 1) % cycle) + 1
      : undefined;
  const nextDay =
    rules.next_milestone_in_cycle ??
    (rules.milestones ?? [])
      .map(([d]) => d)
      .filter((d) => d > (pos ?? streakNow))
      .sort((a, b) => a - b)[0] ??
    null;
  const ms = (rules.milestones ?? []).slice().sort((a, b) => a[0] - b[0]);
  return (
    <div className="ck-rules">
      <h4 className="ck-rules__title">
        {t.heroMsTitle}
        {cycle ? (
          <span className="ck-rules__pos">
            {t.heroMsNow.replace("{pos}", String(pos ?? 1))}
          </span>
        ) : null}
      </h4>
      <ol className="ck-rules__chain">
        {ms.map(([day, bonus]) => {
          const isNext = day === nextDay;
          const isHit =
            nextDay != null
              ? day < nextDay
              : pos != null && day <= pos;
          return (
            <li
              key={day}
              className={
                isNext
                  ? "is-next"
                  : isHit
                    ? "is-hit"
                    : undefined
              }
            >
              <i className="ck-rules__dot" aria-hidden />
              <b>{day}</b>
              <span>
                +{bonus} {currency}
              </span>
              <small>
                {isNext
                  ? t.rulesNextBadge.replace("{day}", String(day))
                  : isHit
                    ? t.heroMsHit
                    : ""}
              </small>
            </li>
          );
        })}
      </ol>
      <p className="ck-rules__note">
        {t.rulesFirst.replace("{n}", String(rules.first))}
        {" · "}
        {t.rulesDaily.replace("{n}", String(rules.streak))}
        {" · "}
        {t.rulesCap.replace("{n}", String(rules.daily_cap))}
      </p>
      <p className="ck-rules__note">{t.rulesBreak}</p>
      {cycle ? (
        <p className="ck-rules__note">
          {t.rulesCycle
            .replace("{cycle}", String(cycle))
            .replace("{pos}", String(pos ?? 1))}
        </p>
      ) : null}
    </div>
  );
}
