"use client";

import { ResubButton } from "@/components/home-resub-button";
import type { DictT, Home2T, HomeData } from "@/components/home-data";

/** 首页签到日历板块（从 home-sections.tsx 按域拆出，300 门禁）：
 *  签到按钮 + 补签卡入口 + 本月日历网格。 */

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
  // 周一开头对齐：本月 1 号之前补空位
  const firstDate = new Date(home.attendance.calendar[0]?.date ?? Date.now());
  const leadingBlanks = (firstDate.getDay() + 6) % 7; // 周一=0
  return (
    <aside className="baozi-panel attendance-card">
      <header className="baozi-panel__head">
        <h2>
          <span aria-hidden="true">📅</span> {t.attendanceTitle}
        </h2>
        <span className="flex items-center gap-2">
          {home.attendance.checked_today ? (
            <span className="attendance-done">{t.attended}</span>
          ) : (
            <button type="button" className="baozi-button" onClick={checkin} disabled={checkinBusy}>
              {checkinBusy ? dict.my.checkinBusy : t.checkinNow}
            </button>
          )}
          <ResubButton cards={home.attendance.makeup_cards ?? 0} />
        </span>
      </header>
      {checkinMsg && <p className="attendance-msg">{checkinMsg}</p>}
      <div className="attendance-card__summary">
        <div>
          <strong>{home.attendance.month}</strong>
          <span>
            {t.streak} {home.attendance.streak} {dict.usercp.days}
          </span>
        </div>
        <div>
          <strong>{home.attendance.total_days}</strong>
          <span>{t.totalDays}</span>
        </div>
      </div>
      <div className="attendance-calendar" aria-label={`${home.attendance.month}${t.calendar}`}>
        {t.weekdays.map((w) => (
          <span key={w} className="attendance-calendar__weekday">
            {w}
          </span>
        ))}
        {Array.from({ length: leadingBlanks }).map((_, i) => (
          <span key={`blank-${i}`} className="attendance-calendar__day is-outside" />
        ))}
        {home.attendance.calendar.map((d) => {
          const todayStr = new Date().toISOString().slice(0, 10);
          const isToday = d.date === todayStr;
          const isFuture = d.date > todayStr;
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
              <small>{d.done ? `+${d.reward}` : isFuture ? "\u00A0" : t.unsignedShort}</small>
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
