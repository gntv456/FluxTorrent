/**
 * 后台用户详情共享类型与常量（从 components/admin-user-detail.tsx 按域拆出）：
 * Detail 全景字段 / 火花流水 / 登录记录 / 做种行 / 标签映射 / 字节与时长格式化。
 */

export interface Detail {
  id: number;
  username: string;
  email: string;
  passkey: string;
  class_id: number;
  class_name: string | null;
  title: string | null;
  uploaded: number;
  downloaded: number;
  spark_balance: number;
  status: number;
  download_enabled: boolean;
  suspended: boolean;
  parked: boolean;
  donor: boolean;
  totp_enabled: boolean;
  invited_by: number | null;
  inviter_name: string | null;
  created_at: string;
  last_seen_at: string | null;
  seeding: number;
  leeching: number;
  uploads: number;
  invites_unused: number;
  comments: number;
  downloaded_count: number;
  medals: number;
  warned_until: string | null;
  warned_reason: string | null;
  last_ip: string | null;
  seed_seconds: number;
  attendance_days: number;
}

export interface SparkRow {
  id: number;
  username: string;
  amount: number;
  kind: string;
  balance_after: number;
  created_at: string;
}
export interface LoginRow {
  id: number;
  username: string;
  ip: string;
  ok: boolean;
  created_at: string;
}
export interface SeedRow {
  torrent_id: number;
  name: string;
  size: number;
  seeded_seconds: number;
  seeding: boolean;
  hr_flag: boolean;
}

/** 用户状态显示名（按 status 作下标）；三语在 i18n `userDetail.statusLabels` */
export const statusLabels = (labels?: string[]): string[] => labels ?? [];

export const PER_PAGE = 15;

export function fmtBytes(n: number): string {
  if (n >= 1099511627776) return `${(n / 1099511627776).toFixed(2)} TB`;
  if (n >= 1073741824) return `${(n / 1073741824).toFixed(2)} GB`;
  if (n >= 1048576) return `${(n / 1048576).toFixed(2)} MB`;
  return `${(n / 1024).toFixed(2)} KB`;
}
/** 单位由调用方传 i18n `userDetail.hoursUnit`；缺省 "h"（语言中性） */
export const fmtHours = (sec: number, unit = "h") =>
  `${Math.floor(sec / 3600)} ${unit}`;

/** 权限分类 → 显示名（与后端 permissions.category 对应）；
 *  三语在 i18n `userDetail.categoryLabels`，取不到回落原 key。 */
export const categoryLabels = (
  labels?: Record<string, string>,
): Record<string, string> => labels ?? {};

/** 道具 kind → 显示名（下拉分组用）；三语在 `userDetail.itemKindLabels` */
export const itemKindLabels = (
  labels?: Record<string, string>,
): Record<string, string> => labels ?? {};

/** 后台详情 tab 键（资料全景 / 火花流水 / 登录记录 / 做种下载） */
export type DetailTab = "profile" | "spark" | "logins" | "seeding";
