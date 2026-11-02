"use client";

import { useState } from "react";
import { ResubButton } from "@/components/home-resub-button";
import { BottomSheet } from "@/components/bottom-sheet";
import { Icon } from "@/components/icons";
import dynamic from "next/dynamic";
import type { DictT, Home2T, HomeData } from "@/components/home-data";

/** 首页签到日历板块（从 home-sections.tsx 按域拆出，300 门禁）：
 *  桌面 = 本月网格卡；手机 = 首屏一条签到条 + 抽屉里的万年历月历。 */

interface AttendanceShape {
  month: string;
  streak: number;
  total_days: number;
  checked_today: boolean;
  calendar: { date: string; day: number; done: boolean; reward: number }[];
  makeup_cards?: number;
}

function attOf(home: HomeData): AttendanceShape | null {
  const a = home.attendance as AttendanceShape & { off?: boolean };
  if (!a || a.off || !Array.isArray(a.calendar)) return null;
  return a;
}

/** 站点今天（UTC+8 日界，与后端生成日历同口径）。 */
function todayStr(): string {
  return new Date(Date.now() + 8 * 3600 * 1000).toISOString().slice(0, 10);
}

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
  // 周一开头对齐：本月 1 号之前补空位（周一=0）
  const firstDate = new Date(att?.calendar[0]?.date ?? Date.now());
  const leadingBlanks = (firstDate.getDay() + 6) % 7;
  // ZT2（2026-10-03）：站点时区（UTC+8）的今天。此前在 map 里用
  // `new Date().toISOString().slice(0,10)`（UTC）——对 UTC+8 用户，每天
  // 00:00–08:00 会算成「昨天」，把今天的格子标成未来/漏签。改为固定 +8 偏移。
  const today = todayStr();
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
      <div className="attendance-card__summary">
        <div>
          <strong>{att?.month}</strong>
          <span>
            {t.streak} {att?.streak} {dict.usercp.days}
          </span>
        </div>
        <div>
          <strong>{att?.total_days}</strong>
          <span>{t.totalDays}</span>
        </div>
      </div>
      <div
        className="attendance-calendar"
        aria-label={`${att?.month}${t.calendar}`}
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
        {att?.calendar.map((d) => {
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

/** 手机端首屏签到条（79px 一条）：签到主动作 + 连签概况 + 下一档里程碑。
 *  整月日历与补签都收进抽屉（AttendanceSheet），首屏不再常驻 490px。 */
export function AttendanceStrip({
  home,
  t,
  checkin,
  checkinBusy,
  onOpenCalendar,
}: {
  home: HomeData;
  t: Home2T;
  checkin: () => Promise<void>;
  checkinBusy: boolean;
  onOpenCalendar: () => void;
}) {
  const att = attOf(home);
  if (!att) return null;
  const ms = home.attendance_milestones ?? [];
  const next = ms.find((m) => m.day > att.streak);
  const prevMilestone = [...ms]
    .reverse()
    .find((m) => m.day <= att.streak);
  const from = prevMilestone?.day ?? 0;
  const span = next ? next.day - from : 1;
  const pct = next
    ? Math.max(0, Math.min(100, ((att.streak - from) / span) * 100))
    : 0;
  return (
    <div className="mhome-strip md:hidden">
      <span className="mhome-strip__ico" aria-hidden="true">
        <Icon name="clock" size={19} />
      </span>
      <button
        type="button"
        className="mhome-strip__txt"
        onClick={onOpenCalendar}
      >
        <b>{t.attendanceTitle}</b>
        <i>
          {att.checked_today
            ? t.attended
            : t.mhomeUnchecked}
          {" · "}
          {t.streak} {att.streak} {t.days}
          {" · "}
          {t.totalShort} {att.total_days} {t.days}
        </i>
        {next ? (
          <span className="mhome-strip__ms">
            <u>
              <i style={{ width: `${pct}%` }} />
            </u>
            <em>
              {t.milestoneNext.replace("{day}", String(next.day))}
              {" "}
              <b>
                +{next.value} {t.sparkShort}
              </b>
            </em>
          </span>
        ) : null}
      </button>
      {att.checked_today ? (
        <span className="mhome-strip__btn is-done">{t.attended}</span>
      ) : (
        <button
          type="button"
          className="mhome-strip__btn"
          onClick={checkin}
          disabled={checkinBusy}
        >
          {t.checkinNow}
        </button>
      )}
    </div>
  );
}

/** 手机端签到日历抽屉：带万年历的月历 + 选中日明细 + 补签口。 */
/** 万年历面板懒加载：lunar-javascript 的表 300KB，抽屉不开就不下载。 */
const AlmanacSection = dynamic(
  () =>
    import("@/components/home-almanac-panel").then((m) => m.AlmanacSection),
  { ssr: false, loading: () => <p className="mhome-sheet__note">…</p> },
);

export function AttendanceSheet({
  open,
  onClose,
  home,
  t,
  checkin,
  checkinBusy,
  checkinMsg,
}: {
  open: boolean;
  onClose: () => void;
  home: HomeData;
  t: Home2T;
  checkin: () => Promise<void>;
  checkinBusy: boolean;
  checkinMsg: string | null;
}) {
  const att = attOf(home);
  const today = todayStr();
  const [cursor, setCursor] = useState(() => {
    const n = new Date(Date.now() + 8 * 3600 * 1000);
    return { y: n.getUTCFullYear(), m: n.getUTCMonth() + 1 };
  });
  const [picked, setPicked] = useState(today);
  const doneMap: Record<string, number> = {};
  att?.calendar.forEach((d) => {
    if (d.done) doneMap[d.date] = d.reward;
  });
  return (
    <BottomSheet open={open} onClose={onClose} title={t.attendanceTitle}>
      {att ? (
        <>
          <p className="mhome-sheet__note">
            {t.mhomeNote
              .replace("{streak}", String(att.streak))
              .replace("{total}", String(att.total_days))
              .replace("{cards}", String(att.makeup_cards ?? 0))}
          </p>
          <div className="mhome-sheet__actions">
            {att.checked_today ? (
              <span className="mhome-strip__btn is-done">{t.attended}</span>
            ) : (
              <button
                type="button"
                className="mhome-strip__btn"
                onClick={checkin}
                disabled={checkinBusy}
              >
                {t.checkinNow}
              </button>
            )}
            <ResubButton cards={att.makeup_cards ?? 0} />
            {checkinMsg ? (
              <p className="attendance-msg">{checkinMsg}</p>
            ) : null}
          </div>
          <AlmanacSection
            year={cursor.y}
            month={cursor.m}
            selected={picked}
            today={today}
            doneMap={doneMap}
            onMonth={(y, m) => setCursor({ y, m })}
            onPick={setPicked}
          />
        </>
      ) : null}
    </BottomSheet>
  );
}
