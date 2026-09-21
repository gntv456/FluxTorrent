/**
 * 任务定义·共享类型与常量（从 components/admin-tasks.tsx 按域拆出）：
 * 任务行类型、空白表单值、任务种类/周期字典与时间本地化工具。
 */

export interface TaskRow {
  id: number;
  name: string;
  metric: Record<string, unknown>;
  starts_at: string;
  ends_at: string;
  target_class: number;
  reward: number;
  penalty: number;
  claim_limit: number | null;
  kind: string;
  auto_assign: boolean;
  period: string;
  /** 以下为 0093 考核引擎字段（本次补齐为可配） */
  duration_days: number;
  subtitle: string | null;
  tier: string | null;
  fee: number;
  quota_total: number;
  sort: number;
}

export const EMPTY = {
  name: "",
  metric: "{}",
  starts_at: "",
  ends_at: "",
  target_class: "0",
  reward: "0",
  penalty: "0",
  claim_limit: "",
  kind: "task",
  auto_assign: false,
  period: "once",
  duration_days: "30",
  subtitle: "",
  tier: "",
  fee: "0",
  quota_total: "200",
  sort: "0",
};

export const KINDS = [
  { v: "task", label: "普通任务" },
  { v: "onboard", label: "新人转正考核" },
  { v: "periodic", label: "周期考核" },
];

export const PERIODS = [
  { v: "once", label: "一次性" },
  { v: "monthly", label: "每月" },
  { v: "quarterly", label: "每季" },
];

export function toLocalInput(iso: string): string {
  const d = new Date(iso);
  return new Date(d.getTime() - d.getTimezoneOffset() * 60000)
    .toISOString()
    .slice(0, 16);
}
