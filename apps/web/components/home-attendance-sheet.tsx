"use client";

import { useEffect, useState } from "react";
import dynamic from "next/dynamic";
import { api } from "@/lib/api-client";
import { ResubButton } from "@/components/home-resub-button";
import { BottomSheet } from "@/components/bottom-sheet";
import type { Home2T, HomeData } from "@/components/home-data";
import { attOf, todayStr } from "@/components/home-attendance-shared";

/** 万年历面板懒加载：lunar-javascript 的表 300KB，抽屉不开就不下载。 */
const AlmanacSection = dynamic(
  () =>
    import("@/components/home-almanac-panel").then((m) => m.AlmanacSection),
  { ssr: false, loading: () => <p className="mhome-sheet__note">…</p> },
);

/** 手机端签到日历抽屉：带万年历的月历 + 选中日明细 + 补签口。 */
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
  // 逐月签到标记（doneMap）：当月用后端实时 att；翻到其它月拉 /attendance/calendar
  const [doneMap, setDoneMap] = useState<Record<string, number>>({});
  const curY = Number(today.slice(0, 4));
  const curM = Number(today.slice(5, 7));
  const isCurrentMonth = cursor.y === curY && cursor.m === curM;
  useEffect(() => {
    if (!open) return;
    if (isCurrentMonth) {
      const m: Record<string, number> = {};
      att?.calendar.forEach((d) => {
        if (d.done) m[d.date] = d.reward;
      });
      setDoneMap(m);
      return;
    }
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
  }, [open, cursor.y, cursor.m, isCurrentMonth, att]);
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
