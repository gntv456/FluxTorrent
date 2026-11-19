/**
 * 体验度量（C4）：轻量客户端埋点，度量首屏可用 / LCP / 漏斗 / 回访。
 * 数据落 localStorage 供用户自查与 E2E 读取；不新增后端端点，零网络开销。
 * 生产如需汇总，只需把写入换成现有日志通道，调用点不用改。
 */
export type UxMetricName =
  | "app_ready"
  | "lcp"
  | "empty_state"
  | "first_torrent_open"
  | "guide_open"
  | "checkin"
  | "publish_start"
  | "invite_issue"
  | "filter_save"
  | "search_submit"
  | "coldstart_task_open"
  | "report_open"
  | "signup_complete";

const UX_EVENTS = new Set<UxMetricName>([
  "app_ready", "lcp", "empty_state", "first_torrent_open",
  "guide_open", "checkin", "publish_start", "invite_issue",
  "filter_save", "search_submit", "coldstart_task_open",
  "report_open", "signup_complete",
]);

export function isUxMetricName(v: string): v is UxMetricName {
  return UX_EVENTS.has(v as UxMetricName);
}

export interface UxMetric {
  name: UxMetricName;
  value: number;
  at: number;
  label?: string;
}

/** 本地画像：首访时间 + 活跃日集合 + 事件流。
 *  只存时间戳与行为类型，不含姓名、路径或任何内容。 */
export interface UxProfile {
  firstSeen: number | null;
  activeDays: string[];
  events: UxMetric[];
}

const KEY = "flux-ux-metrics";
const FIRST_KEY = "flux-ux-first-seen";
const DAYS_KEY = "flux-ux-active-days";
const MAX = 300;
const DAY_CAP = 90;

function localDay(at: number): string {
  const d = new Date(at - new Date().getTimezoneOffset() * 60000);
  return d.toISOString().slice(0, 10);
}

function touchPresence(at: number): void {
  try {
    const rawFirst = localStorage.getItem(FIRST_KEY);
    const first = rawFirst ? Number(rawFirst) : NaN;
    if (!Number.isFinite(first) || first <= 0) {
      localStorage.setItem(FIRST_KEY, String(at));
    }
    const rawDays = localStorage.getItem(DAYS_KEY);
    const days: string[] = rawDays ? (JSON.parse(rawDays) as string[]) : [];
    const day = localDay(at);
    if (!days.includes(day)) {
      days.push(day);
      localStorage.setItem(DAYS_KEY, JSON.stringify(days.slice(-DAY_CAP)));
    }
  } catch {
    /* 隐私模式：不记忆，不影响当前会话 */
  }
}

export function uxMark(
  name: UxMetricName,
  label?: string,
  value?: number,
): void {
  if (typeof window === "undefined") return;
  const at = Date.now();
  const m: UxMetric = {
    name,
    value: value ?? Math.round(performance.now()),
    at,
    ...(label ? { label } : {}),
  };
  try {
    const raw = localStorage.getItem(KEY);
    const arr: UxMetric[] = raw ? (JSON.parse(raw) as UxMetric[]) : [];
    arr.push(m);
    const kept = arr.slice(-MAX);
    localStorage.setItem(KEY, JSON.stringify(kept));
    touchPresence(at);
    const w = window as typeof window & { __fluxUxMetrics?: UxMetric[] };
    w.__fluxUxMetrics = kept;
  } catch {
    /* 同上 */
  }
}

export function uxProfile(): UxProfile {
  if (typeof window === "undefined") {
    return { firstSeen: null, activeDays: [], events: [] };
  }
  try {
    const rawFirst = localStorage.getItem(FIRST_KEY);
    const first = rawFirst ? Number(rawFirst) : NaN;
    const rawDays = localStorage.getItem(DAYS_KEY);
    const rawEvents = localStorage.getItem(KEY);
    return {
      firstSeen: Number.isFinite(first) && first > 0 ? first : null,
      activeDays: rawDays ? (JSON.parse(rawDays) as string[]) : [],
      events: rawEvents ? (JSON.parse(rawEvents) as UxMetric[]) : [],
    };
  } catch {
    return { firstSeen: null, activeDays: [], events: [] };
  }
}

/** 兼容旧调用：只读事件数组 */
export function uxRead(): UxMetric[] {
  return uxProfile().events;
}

/** 数据可携带 / 清除：用户对自己的行为数据有完整控制权 */
export function uxClear(): void {
  if (typeof window === "undefined") return;
  try {
    localStorage.removeItem(KEY);
    localStorage.removeItem(FIRST_KEY);
    localStorage.removeItem(DAYS_KEY);
    const w = window as typeof window & { __fluxUxMetrics?: UxMetric[] };
    w.__fluxUxMetrics = [];
  } catch {
    /* 同上 */
  }
}
