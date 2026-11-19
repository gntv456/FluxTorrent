import type { UxMetric, UxMetricName, UxProfile } from "./ux-metrics";

/** 指标聚合器（纯函数，无副作用、无 window 依赖）。
 *  把本地事件流折算成产品指标体系里的口径，
 *  供 UI 自查、E2E 复算与服务端接入共用同一份算法。 */

export interface UxFunnelStep {
  name: UxMetricName;
  count: number;
  firstAt: number | null;
}

export interface UxReport {
  firstSeenAt: number | null;
  ageDays: number;
  activeDays: number;
  d1Return: boolean;
  d7Return: boolean;
  steps: UxFunnelStep[];
  ttfvMs: number | null;
  emptyByLabel: { label: string; count: number }[];
  searchCount: number;
  eventCount: number;
}

const FUNNEL: UxMetricName[] = [
  "signup_complete",
  "app_ready",
  "guide_open",
  "first_torrent_open",
  "checkin",
  "publish_start",
  "invite_issue",
];

function dayStr(ms: number): string {
  return new Date(ms).toISOString().slice(0, 10);
}

function shiftDay(day: string, delta: number): string {
  const t = Date.parse(day + "T00:00:00Z") + delta * 86400000;
  return dayStr(t);
}

function firstOf(events: UxMetric[], name: UxMetricName): number | null {
  let min = Infinity;
  for (const e of events) {
    if (e.name === name && e.at < min) min = e.at;
  }
  return Number.isFinite(min) ? min : null;
}

export function computeUxReport(
  profile: UxProfile,
  now: number = Date.now(),
): UxReport {
  const events = Array.isArray(profile.events) ? profile.events : [];
  const days = Array.isArray(profile.activeDays) ? profile.activeDays : [];
  const first = profile.firstSeen;
  const ageDays =
    first != null && Number.isFinite(first)
      ? Math.max(0, Math.floor((now - first) / 86400000))
      : 0;
  const baseDay =
    first != null && Number.isFinite(first) ? dayStr(first) : null;
  const d1 = baseDay ? days.includes(shiftDay(baseDay, 1)) : false;
  const d7 = baseDay ? days.includes(shiftDay(baseDay, 7)) : false;

  const steps: UxFunnelStep[] = FUNNEL.map((name) => {
    let count = 0;
    for (const e of events) if (e.name === name) count++;
    return { name, count, firstAt: firstOf(events, name) };
  });

  const ready = firstOf(events, "app_ready");
  const opened = firstOf(events, "first_torrent_open");
  const ttfvMs =
    ready != null && opened != null && opened >= ready
      ? opened - ready
      : null;

  const byLabel = new Map<string, number>();
  let searchCount = 0;
  for (const e of events) {
    if (e.name === "empty_state") {
      const key = e.label || "unknown";
      byLabel.set(key, (byLabel.get(key) ?? 0) + 1);
    }
    if (e.name === "search_submit") searchCount++;
  }
  const emptyByLabel = [...byLabel.entries()]
    .map(([label, count]) => ({ label, count }))
    .sort((a, b) => b.count - a.count);

  return {
    firstSeenAt: first ?? null,
    ageDays,
    activeDays: days.length,
    d1Return: d1,
    d7Return: d7,
    steps,
    ttfvMs,
    emptyByLabel,
    searchCount,
    eventCount: events.length,
  };
}
