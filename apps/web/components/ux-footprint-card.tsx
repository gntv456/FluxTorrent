"use client";

import { useEffect, useMemo, useState } from "react";
import { useI18n } from "@/i18n/client";
import {
  uxClear,
  uxProfile,
  type UxMetricName,
  type UxProfile,
} from "@/lib/ux-metrics";
import { computeUxReport } from "@/lib/ux-metrics-report";

const STEP_LABEL: Record<string, string> = {
  signup_complete: "stepSignup",
  app_ready: "stepReady",
  guide_open: "stepGuide",
  first_torrent_open: "stepOpen",
  checkin: "stepCheckin",
  publish_start: "stepPublish",
  invite_issue: "stepInvite",
};

/** 我的使用足迹：把本地行为数据变成用户可见、可导出的资产。
 *  没有后端、不上传；这是数据可携带端点。 */
export function UxFootprintCard() {
  const { dict, locale } = useI18n();
  const u = dict.uxOpt;
  const [profile, setProfile] = useState<UxProfile | null>(null);
  const [confirmClear, setConfirmClear] = useState(false);
  const [cleared, setCleared] = useState(false);

  useEffect(() => {
    setProfile(uxProfile());
  }, []);

  const empty = { firstSeen: null, activeDays: [], events: [] };
  const report = useMemo(
    () => computeUxReport(profile ?? empty),
    [profile],
  );

  if (!profile || report.eventCount === 0) {
    return (
      <section className="ux-foot" aria-label={u.footprintTitle}>
        <h2 className="ux-foot__title">{u.footprintTitle}</h2>
        <p className="ux-foot__empty">{u.footprintEmpty}</p>
      </section>
    );
  }

  function exportJson() {
    const payload = {
      schema: "flux-ux-footprint/v1",
      exportedAt: new Date().toISOString(),
      locale,
      report,
    };
    const blob = new Blob([JSON.stringify(payload, null, 2)], {
      type: "application/json",
    });
    const url = URL.createObjectURL(blob);
    const a = document.createElement("a");
    a.href = url;
    a.download = "flux-ux-footprint.json";
    a.click();
    URL.revokeObjectURL(url);
  }

  const stats: [string, string][] = [
    [u.footprintActive, String(report.activeDays)],
    [u.footprintEvents, String(report.eventCount)],
    [u.footprintD1, report.d1Return ? u.footprintYes : u.footprintNo],
    [u.footprintD7, report.d7Return ? u.footprintYes : u.footprintNo],
  ];

  return (
    <section className="ux-foot" aria-label={u.footprintTitle}>
      <h2 className="ux-foot__title">{u.footprintTitle}</h2>
      <p className="ux-foot__desc">{u.footprintDesc}</p>
      <dl className="ux-foot__stats">
        {stats.map(([k, v]) => (
          <div key={k} className="ux-foot__stat">
            <dt>{k}</dt>
            <dd>{v}</dd>
          </div>
        ))}
      </dl>
      <h3 className="ux-foot__sub">{u.footprintFunnel}</h3>
      <ul className="ux-foot__steps">
        {report.steps.map((s) => (
          <li key={s.name} className="ux-foot__step">
            <span>
              {u[STEP_LABEL[s.name] as keyof typeof u]}
            </span>
            <b>{s.count}</b>
          </li>
        ))}
      </ul>
      <div className="ux-foot__actions">
        <button type="button" className="ux-foot__btn" onClick={exportJson}>
          {u.footprintExport}
        </button>
        <button
          type="button"
          className="ux-foot__btn ux-foot__btn--quiet"
          onClick={() => {
            if (!confirmClear) {
              setConfirmClear(true);
              return;
            }
            uxClear();
            setProfile(null);
            setCleared(true);
            setConfirmClear(false);
          }}
        >
          {confirmClear ? u.footprintClearConfirm : u.footprintClear}
        </button>
        {cleared ? (
          <span role="status" className="ux-foot__cleared">
            {u.footprintCleared}
          </span>
        ) : null}
      </div>
    </section>
  );
}
