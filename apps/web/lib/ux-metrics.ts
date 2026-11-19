/**
 * 体验度量（C4）：轻量客户端埋点，度量首屏可用 / LCP / 空态曝光。
 * 数据落 localStorage 供本地与 E2E 读取；不新增后端端点，零网络开销。
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
  | "report_open";

const UX_EVENTS = new Set<UxMetricName>([
  "app_ready", "lcp", "empty_state", "first_torrent_open",
  "guide_open", "checkin", "publish_start", "invite_issue",
  "filter_save", "search_submit", "coldstart_task_open",
  "report_open",
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
const KEY = "flux-ux-metrics";
const MAX = 60;

export function uxMark(
  name: UxMetricName,
  label?: string,
  value?: number,
): void {
  if (typeof window === "undefined") return;
  const m: UxMetric = {
    name,
    value: value ?? Math.round(performance.now()),
    at: Date.now(),
    ...(label ? { label } : {}),
  };
  try {
    const raw = localStorage.getItem(KEY);
    const arr: UxMetric[] = raw ? (JSON.parse(raw) as UxMetric[]) : [];
    arr.push(m);
    const kept = arr.slice(-MAX);
    localStorage.setItem(KEY, JSON.stringify(kept));
    const w = window as typeof window & { __fluxUxMetrics?: UxMetric[] };
    w.__fluxUxMetrics = kept;
  } catch {
    /* 隐私模式下不可用：静默降级，不打扰用户 */
  }
}

export function uxRead(): UxMetric[] {
  if (typeof window === "undefined") return [];
  try {
    return JSON.parse(localStorage.getItem(KEY) ?? "[]") as UxMetric[];
  } catch {
    return [];
  }
}
