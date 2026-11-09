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

const AlmanacSection = dynamic(
  () =>
    import("@/components/home-almanac-panel").then((m) => m.AlmanacSection),
  { ssr: false, loading: () => <p className="attendance-msg">…</p> },
);

interface AttendanceStatus {
  checked_today: boolean;
  streak: number;
  recent: [string, number, number][]; // [date, streak, reward]
  makeup_cards: number;
}

function todayStr(): string {
  return new Date(Date.now() + 8 * 3600 * 1000).toISOString().slice(0, 10);
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

  // 状态（连签/累计/补签卡/今日是否已签）
  const loadStatus = () => {
    api
      .get<AttendanceStatus & { total_days?: number }>("/api/v1/attendance")
      .then((s) => {
        setStatus(s);
        setTotalDays((s as { total_days?: number }).total_days ?? s.recent?.length ?? 0);
      })
      .catch(() => {});
  };
  useEffect(loadStatus, []);

  // 逐月签到标记
  const isCurrentMonth =
    cursor.y === Number(today.slice(0, 4)) && cursor.m === Number(today.slice(5, 7));
  useEffect(() => {
    let alive = true;
    api
      .get<{ ym: string; days: { date: string; done: boolean; reward: number }[] }>(
        `/api/v1/attendance/calendar?ym=${cursor.y}-${String(cursor.m).padStart(2, "0")}`,
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
      setMsg(e instanceof ApiError ? e.message : t.checkinFail);
    } finally {
      setBusy(false);
    }
  }

  return (
    <div className="checkin-page">
      <div className="checkin-page__summary">
        <div className="checkin-page__stat">
          <strong>{status?.streak ?? 0}</strong>
          <span>{t.streak}</span>
        </div>
        <div className="checkin-page__stat">
          <strong>{totalDays}</strong>
          <span>{t.totalDays}</span>
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
      <div className="checkin-page__almanac">
        <AlmanacSection
          year={cursor.y}
          month={cursor.m}
          selected={picked}
          today={today}
          doneMap={doneMap}
          onMonth={(y, m) => setCursor({ y, m })}
          onPick={setPicked}
        />
      </div>
    </div>
  );
}
