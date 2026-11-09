"use client";

import { Icon } from "@/components/icons";
import type { Home2T, HomeData } from "@/components/home-data";
import { attOf } from "@/components/home-attendance-shared";

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
  const prevMilestone = [...ms].reverse().find((m) => m.day <= att.streak);
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
          {att.checked_today ? t.attended : t.mhomeUnchecked}
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
              {t.milestoneNext.replace("{day}", String(next.day))}{" "}
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
