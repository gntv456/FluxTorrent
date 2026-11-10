"use client";

import { useEffect, useState } from "react";
import dynamic from "next/dynamic";
import { api, ApiError } from "@/lib/api-client";
import { useI18n } from "@/i18n/client";
import { ResubButton } from "@/components/home-resub-button";

/** 桌面签到页（带万年历）：主动作 + 连签/累计/补签卡概况 + 可翻月的万年历月历
 *  （与手机抽屉同一 AlmanacSection；lunar 包 300KB，按需 dynamic 加载）。
 *
 *  逐月签到标记走 GET /attendance/calendar?ym=YYYY-MM（翻到任意历史月都能看到
 *  当月签到，补签行也计入）；当月用 /attendance 的 recent 实时态。 */

const AlmanacCalendar = dynamic(
  () =>
    import("@/components/home-almanac-panel").then((m) => m.AlmanacCalendar),
  { ssr: false, loading: () => <p className="attendance-msg">…</p> },
);
const AlmanacDetailPanel = dynamic(
  () =>
    import("@/components/home-almanac-panel").then(
      (m) => m.AlmanacDetailPanel,
    ),
  { ssr: false, loading: () => <p className="attendance-msg">…</p> },
);

interface AttendanceRules {
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

interface AttendanceStatus {
  checked_today: boolean;
  streak: number;
  recent: [string, number, number][]; // [date, streak, reward]
  makeup_cards: number;
  /** 签到规则（2026-11-10）：与签到主流程同一套 site_settings 参数，
   *  站长改配置这里即跟随——旧版页面只字未提规则，用户连签 27 天
   *  都以为每天固定 8 魔力是坏了（里程碑 10/20/30 天加送根本没人知道）。 */
  rules?: AttendanceRules;
}

function todayStr(): string {
  return new Date(Date.now() + 8 * 3600 * 1000).toISOString().slice(0, 10);
}

/** 规则卡（拆出主组件，300 行门禁）：周期制口径——下一档按周期内位置判定。
 *  streak=483 的人 cycle_pos=3，高亮的是「下一档 10 天」（本周期内），而不是
 *  旧口径的「已过 30 无下一档」。 */
function RulesCard({
  rules,
  streakNow,
}: {
  rules: AttendanceRules;
  streakNow: number;
}) {
  const t = useI18n().dict.home2;
  // 周期口径：后端给了 cycle_pos/next 就直接用；旧后端（缺字段）回落
  // 绝对天数口径，保证灰度期两边都能渲染。
  const cycle = rules.cycle_days;
  const pos =
    cycle && cycle > 0
      ? (((Math.max(streakNow, 1) - 1) % cycle) + 1)
      : undefined;
  const nextDay =
    rules.next_milestone_in_cycle ??
    (rules.milestones ?? [])
      .map(([d]) => d)
      .filter((d) => d > (pos ?? streakNow))
      .sort((a, b) => a - b)[0] ??
    null;
  return (
    <div className="checkin-page__rules">
      <h4>{t.rulesTitle}</h4>
      <ul>
        <li>{t.rulesFirst.replace("{n}", String(rules.first))}</li>
        <li>{t.rulesDaily.replace("{n}", String(rules.streak))}</li>
        {(rules.milestones ?? [])
          .slice()
          .sort((a, b) => a[0] - b[0])
          .map(([day, bonus]) => {
            const total = Math.min(
              rules.streak + bonus,
              rules.daily_cap,
            );
            const isNext = day === nextDay;
            return (
              <li key={day} className={isNext ? "is-next" : undefined}>
                {isNext && (
                  <span className="checkin-page__rules-next">
                    {t.rulesNextBadge.replace("{day}", String(day))}
                  </span>
                )}
                {t.rulesMilestone
                  .replace("{day}", String(day))
                  .replace("{bonus}", String(bonus))
                  .replace("{total}", String(total))}
              </li>
            );
          })}
        <li>{t.rulesCap.replace("{n}", String(rules.daily_cap))}</li>
        <li>{t.rulesBreak}</li>
        {cycle ? (
          <li>
            {t.rulesCycle
              .replace("{cycle}", String(cycle))
              .replace("{pos}", String(pos ?? 1))}
          </li>
        ) : null}
      </ul>
    </div>
  );
}

export function CheckinPage() {
  const { dict, currency } = useI18n();
  const t = dict.home2;
  const today = todayStr();
  const [status, setStatus] = useState<AttendanceStatus | null>(null);
  const [busy, setBusy] = useState(false);
  const [msg, setMsg] = useState<string | null>(null);
  const [cursor, setCursor] = useState(() => ({
    y: Number(today.slice(0, 4)),
    m: Number(today.slice(5, 7)),
  }));
  const [picked, setPicked] = useState(today);
  const [doneMap, setDoneMap] = useState<Record<string, number>>({});
  const [totalDays, setTotalDays] = useState(0);
  // 规则卡高亮「下一档里程碑」用：以已展示的连签数为基线（签到成功会随 status 刷新）
  const streakNow = status?.streak ?? 0;

  // 状态（连签/累计/补签卡/今日是否已签）；失败静默但把已签态打捞回来——
  // POST /checkin 撞幂等（1002「今天已经签到过啦」）说明库里今天已有行，
  // 弱网下 GET 状态失败会让页面停留在「未签」初始态（SSR 直出 0/0），
  // 用户看到的就是「已签到但日历未亮」（2026-11-10 线上报障）。
  const loadStatus = () => {
    api
      .get<AttendanceStatus & { total_days?: number }>("/api/v1/attendance")
      .then((s) => {
        setStatus(s);
        setTotalDays(
          (s as { total_days?: number }).total_days ??
            s.recent?.length ??
            0,
        );
      })
      .catch(() => {});
  };
  useEffect(loadStatus, []);

  // 逐月签到标记
  const isCurrentMonth =
    cursor.y === Number(today.slice(0, 4)) &&
    cursor.m === Number(today.slice(5, 7));
  useEffect(() => {
    let alive = true;
    api
      .get<{
        ym: string;
        days: { date: string; done: boolean; reward: number }[];
      }>(
        `/api/v1/attendance/calendar?ym=${cursor.y}-` +
          `${String(cursor.m).padStart(2, "0")}`,
      )
      .then((r) => {
        if (!alive) return;
        const m: Record<string, number> = {};
        (r?.days ?? []).forEach((d) => {
          if (d.done) m[d.date] = d.reward;
        });
        setDoneMap(m);
      })
      .catch(() => {
        if (alive) setDoneMap({});
      });
    return () => {
      alive = false;
    };
  }, [cursor.y, cursor.m]);

  async function checkin() {
    if (busy || status?.checked_today) return;
    setBusy(true);
    setMsg(null);
    try {
      const r = await api.post<{ reward: number; streak: number }>(
        "/api/v1/attendance/checkin",
        {},
      );
      setMsg(
        dict.my.checkinOk
          .replace("{reward}", String(r.reward))
          .replace("{streak}", String(r.streak))
          .replace("{magic}", currency),
      );
      loadStatus();
      // 今日已签：刷新当月标记
      if (isCurrentMonth) {
        setDoneMap((prev) => ({ ...prev, [today]: r.reward }));
      }
    } catch (e) {
      // 撞幂等=库里已有今日行：视为已签成功，反查真实状态点亮日历
      // （而不是让按钮回弹成可点——用户会反复点并一直看到「失败」）。
      if (e instanceof ApiError && e.code === 1002) {
        loadStatus();
        setDoneMap((prev) => ({ ...prev, [today]: prev[today] ?? 0 }));
      } else {
        setMsg(e instanceof ApiError ? e.message : t.checkinFail);
      }
    } finally {
      setBusy(false);
    }
  }

  return (
    <div className="checkin-page">
      <div className="checkin-page__summary">
        <div className="checkin-page__stat">
          <span className="checkin-page__stat-ico" aria-hidden>🔥</span>
          <div>
            <strong>{status?.streak ?? 0}</strong>
            <span>{t.streak}</span>
          </div>
        </div>
        <div className="checkin-page__stat">
          <span className="checkin-page__stat-ico" aria-hidden>📅</span>
          <div>
            <strong>{totalDays}</strong>
            <span>{t.totalDays}</span>
          </div>
        </div>
        <div className="checkin-page__actions">
          {status?.checked_today ? (
            <span className="attendance-done">{t.attended}</span>
          ) : (
            <button
              type="button"
              className="brand-button"
              onClick={checkin}
              disabled={busy}
            >
              {busy ? dict.my.checkinBusy : t.checkinNow}
            </button>
          )}
          <ResubButton cards={status?.makeup_cards ?? 0} />
        </div>
      </div>
      {msg && <p className="attendance-msg">{msg}</p>}
      {status?.rules ? (
        <RulesCard rules={status.rules} streakNow={streakNow} />
      ) : null}
      <div className="checkin-page__almanac">
        <div className="checkin-page__cal">
          <AlmanacCalendar
            year={cursor.y}
            month={cursor.m}
            selected={picked}
            today={today}
            doneMap={doneMap}
            onMonth={(y, m) => setCursor({ y, m })}
            onPick={setPicked}
          />
        </div>
        <aside className="checkin-page__detail">
          <h3 className="checkin-page__detail-title">
            {picked} · 黄历
          </h3>
          <AlmanacDetailPanel date={picked} />
        </aside>
      </div>
    </div>
  );
}
