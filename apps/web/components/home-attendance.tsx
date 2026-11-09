"use client";

import { useEffect, useState } from "react";
import { api } from "@/lib/api-client";
import { ResubButton } from "@/components/home-resub-button";
import { Icon } from "@/components/icons";
import type { DictT, Home2T, HomeData } from "@/components/home-data";
import {
  attOf,
  todayStr,
  type AttendanceShape,
} from "@/components/home-attendance-shared";

/** 年月标签走 i18n 字典，避免组件内硬编码中文。 */
function ymLabel(t: Home2T, y: number, m: number): string {
  return t.yearMonth
    .replace("{y}", String(y))
    .replace("{m}", String(m));
}

/** 首页签到日历板块（从 home-sections.tsx 按域拆出，300 门禁）：
 *  桌面 = 本月网格卡；手机 = 首屏一条签到条 + 抽屉里的万年历月历。
 *  签到条与抽屉已拆至 home-attendance-strip / home-attendance-sheet。 */
export { AttendanceStrip } from "@/components/home-attendance-strip";
export { AttendanceSheet } from "@/components/home-attendance-sheet";

export function AttendanceCard({
  home,
  t,
  dict,
  checkin,
  checkinBusy,
  checkinMsg,
}: {
  home: HomeData;
  t: Home2T;
  dict: DictT;
  checkin: () => Promise<void>;
  checkinBusy: boolean;
  checkinMsg: string | null;
}) {
  const att = attOf(home);
  // 站点时区（UTC+8）的今天
  const today = todayStr();
  // 翻月：cursor 指向当前展示的年月；days 为对应月逐日签到（初始用后端当月）
  const [cursor, setCursor] = useState(() => {
    const [y, m] = (att?.month ?? "").match(/(\d{4})年(\d{1,2})月/)
      ? [
          Number((att!.month.match(/(\d{4})年/) ?? [])[1]),
          Number((att!.month.match(/年(\d{1,2})月/) ?? [])[1]),
        ]
      : [Number(today.slice(0, 4)), Number(today.slice(5, 7))];
    return { y, m };
  });
  const [days, setDays] = useState<AttendanceShape["calendar"]>(
    att?.calendar ?? [],
  );
  const isCurrentMonth =
    cursor.y === Number(today.slice(0, 4)) &&
    cursor.m === Number(today.slice(5, 7));
  // 翻到非当月时拉按月数据；回到当月用后端实时 att（含今日最新状态）
  const [loading, setLoading] = useState(false);
  useEffect(() => {
    if (isCurrentMonth) {
      setDays(att?.calendar ?? []);
      return;
    }
    let alive = true;
    setLoading(true);
    api
      .get<{ ym: string; days: AttendanceShape["calendar"] }>(
        `/api/v1/attendance/calendar?ym=${cursor.y}-` +
          `${String(cursor.m).padStart(2, "0")}`,
      )
      .then((r) => {
        if (alive) setDays(Array.isArray(r?.days) ? r.days : []);
      })
      .catch(() => {
        if (alive) setDays([]);
      })
      .finally(() => {
        if (alive) setLoading(false);
      });
    return () => {
      alive = false;
    };
  }, [cursor.y, cursor.m, isCurrentMonth, att]);
  const goMonth = (delta: number) => {
    setCursor((c) => {
      let y = c.y;
      let m = c.m + delta;
      if (m === 0) {
        y -= 1;
        m = 12;
      } else if (m === 13) {
        y += 1;
        m = 1;
      }
      return { y, m };
    });
  };
  // 周一开头对齐：展示月 1 号之前补空位（周一=0）
  const firstDate = new Date(days[0]?.date ?? `${cursor.y}-${cursor.m}-01`);
  const leadingBlanks = (firstDate.getDay() + 6) % 7;
  return (
    <aside className="brand-panel attendance-card">
      <header className="brand-panel__head">
        <h2>
          <span aria-hidden="true">📅</span> {t.attendanceTitle}
        </h2>
        <span className="flex items-center gap-2">
          {att?.checked_today ? (
            <span className="attendance-done">{t.attended}</span>
          ) : (
            <button
              type="button"
              className="brand-button"
              onClick={checkin}
              disabled={checkinBusy}
            >
              {checkinBusy ? dict.my.checkinBusy : t.checkinNow}
            </button>
          )}
          <ResubButton cards={att?.makeup_cards ?? 0} />
        </span>
      </header>
      {checkinMsg && <p className="attendance-msg">{checkinMsg}</p>}
      <div className="attendance-card__nav">
        <button
          type="button"
          className="attendance-card__mv"
          aria-label={t.prevMonth}
          onClick={() => goMonth(-1)}
        >
          ‹
        </button>
        <strong>{ymLabel(t, cursor.y, cursor.m)}</strong>
        <button
          type="button"
          className="attendance-card__mv"
          aria-label={t.nextMonth}
          disabled={isCurrentMonth}
          onClick={() => goMonth(1)}
        >
          ›
        </button>
      </div>
      <div className="attendance-card__summary">
        <div>
          <strong>
            {t.streak} {att?.streak} {dict.usercp.days}
          </strong>
        </div>
        <div>
          <strong>{att?.total_days}</strong>
          <span>{t.totalDays}</span>
        </div>
      </div>
      <div
        className="attendance-calendar"
        aria-label={`${ymLabel(t, cursor.y, cursor.m)}${t.calendar}`}
      >
        {t.weekdays.map((w) => (
          <span key={w} className="attendance-calendar__weekday">
            {w}
          </span>
        ))}
        {Array.from({ length: leadingBlanks }).map((_, i) => (
          <span
            key={`blank-${i}`}
            className="attendance-calendar__day is-outside"
          />
        ))}
        {days.map((d) => {
          const isToday = d.date === today;
          const isFuture = d.date > today;
          const cls = [
            "attendance-calendar__day",
            d.done ? "is-done" : isFuture ? "is-future" : "is-missed",
            isToday ? "is-today" : "",
          ].join(" ");
          return (
            <span
              key={d.date}
              className={cls}
              title={`${d.date} · ${d.done ? `${t.signed} +${d.reward}` : isFuture ? "" : t.unsigned}`}
            >
              <strong>{d.day}</strong>
              <small>
                {d.done
                  ? `+${d.reward}`
                  : isFuture
                    ? "\u00A0"
                    : t.unsignedShort}
              </small>
            </span>
          );
        })}
      </div>
      <footer className="attendance-card__legend">
        <span>
          <i className="is-done" /> {t.legendDone}
        </span>
        <span>
          <i className="is-today" /> {t.legendToday}
        </span>
      </footer>
    </aside>
  );
}
