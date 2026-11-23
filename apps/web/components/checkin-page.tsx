"use client";

import { useEffect, useState } from "react";
import dynamic from "next/dynamic";
import { withTicketNote, type CheckinResp } from "@/lib/checkin-msg";
import { api, ApiError } from "@/lib/api-client";
import { useI18n } from "@/i18n/client";
import { ResubButton } from "@/components/home-resub-button";
import {
  AttendanceHeatmap,
  type HeatCell,
} from "@/components/attendance-heatmap";
import { CheckinHero } from "@/components/checkin-hero";
import {
  CheckinRules,
  type AttendanceRules,
} from "@/components/checkin-rules";

/** 桌面签到页（重排版 2026-11-20）：hero 主动作卡压顶（今日可得 + 大按钮
 *  + 周期进度刻度），下方两栏——左=万年历（热力足迹并进日历卡尾，降权为
 *  一行），右=黄历明细 + 里程碑规则链。旧版三块平铺（统计条/规则文字卡/
 *  热力条）权重相同、重点不显，本版以「签到能拿多少」为唯一视觉锚点。
 *
 *  逐月签到标记走 GET /attendance/calendar?ym=YYYY-MM（翻到任意历史月都能
 *  看到当月签到，补签行也计入）；当月用 /attendance 的 recent 实时态。 */

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

interface AttendanceStatus {
  checked_today: boolean;
  streak: number;
  recent: [string, number, number][]; // [date, streak, reward]
  makeup_cards: number;
  /** 签到规则（2026-11-10）：与签到主流程同一套 site_settings 参数，
   *  站长改配置这里即跟随。 */
  rules?: AttendanceRules;
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
  // 签到足迹热力（0498）：与 status 同源端点（GET /attendance 的 heatmap）
  const [heat, setHeat] = useState<HeatCell[]>([]);

  // 状态（连签/累计/补签卡/今日是否已签）；失败静默但把已签态打捞回来——
  // POST /checkin 撞幂等（1002「今天已经签到过啦」）说明库里今天已有行，
  // 弱网下 GET 状态失败会让页面停留在「未签」初始态（SSR 直出 0/0），
  // 用户看到的就是「已签到但日历未亮」（2026-11-10 线上报障）。
  const loadStatus = () => {
    api
      .get<AttendanceStatus & { total_days?: number; heatmap?: HeatCell[] }>(
        "/api/v1/attendance",
      )
      .then((s) => {
        setStatus(s);
        setTotalDays(
          (s as { total_days?: number }).total_days ??
            s.recent?.length ??
            0,
        );
        setHeat(s.heatmap ?? []);
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
      const r = await api.post<CheckinResp>("/api/v1/attendance/checkin", {});
      const m = withTicketNote(
        dict.my.checkinOk
          .replace("{reward}", String(r.reward))
          .replace("{streak}", String(r.streak))
          .replace("{magic}", currency),
        dict.my,
        r,
      );
      setMsg(m);
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
      <CheckinHero
        checkedToday={status?.checked_today ?? false}
        streak={status?.streak ?? 0}
        totalDays={totalDays}
        makeupCards={status?.makeup_cards ?? 0}
        rules={status?.rules}
        busy={busy}
        msg={msg}
        onCheckin={checkin}
        checkinLabel={t.checkinNow}
        busyLabel={dict.my.checkinBusy}
        attendedLabel={t.attended}
        resub={
          <ResubButton
            cards={status?.makeup_cards ?? 0}
            onCardsChange={(n) =>
              setStatus((s) => (s ? { ...s, makeup_cards: n } : s))
            }
          />
        }
      />
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
          {/* 热力足迹并进日历卡尾（降权）：近 12 个月逐月天数一行带过 */}
          {heat.length > 0 ? (
            <div className="checkin-page__heat">
              <span className="checkin-page__heat-t">
                {dict.checkin.heat.title}
                <small> {dict.checkin.heat.hint}</small>
              </span>
              <AttendanceHeatmap cells={heat} embedded />
            </div>
          ) : null}
        </div>
        <aside className="checkin-page__detail">
          <h3 className="checkin-page__detail-title">
            {dict.alm.detailTitle.replace("{date}", picked)}
          </h3>
          <AlmanacDetailPanel date={picked} />
          {status?.rules ? (
            <CheckinRules rules={status.rules} streakNow={status.streak ?? 0} />
          ) : null}
        </aside>
      </div>
    </div>
  );
}
