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

export interface SparkRow { id: number; username: string; amount: number; kind: string; balance_after: number; created_at: string }
export interface LoginRow { id: number; username: string; ip: string; ok: boolean; created_at: string }
export interface SeedRow { torrent_id: number; name: string; size: number; seeded_seconds: number; seeding: boolean; hr_flag: boolean }

export const STATUS_LABELS = ["正常", "禁言", "封禁"];
export const PER_PAGE = 15;

export function fmtBytes(n: number): string {
  if (n >= 1099511627776) return `${(n / 1099511627776).toFixed(2)} TB`;
  if (n >= 1073741824) return `${(n / 1073741824).toFixed(2)} GB`;
  if (n >= 1048576) return `${(n / 1048576).toFixed(2)} MB`;
  return `${(n / 1024).toFixed(2)} KB`;
}
export const fmtHours = (sec: number) => `${Math.floor(sec / 3600)} 小时`;

/** 权限分类 → 中文标签（与后端 permissions.category 对应） */
export const CATEGORY_LABEL: Record<string, string> = {
  content: "内容管理",
  liaison: "外联",
  repost: "转载",
  seed: "做种与 H&R",
  user: "用户管理",
  system: "系统",
  site: "站点管理",
  upload: "发布管理",
};

/** 道具 kind → 中文标签（下拉分组用） */
export const ITEM_KIND_LABEL: Record<string, string> = {
  upload_credit: "上传量",
  invite: "邀请类",
  temp_invite: "邀请类",
  gift_spark: "CURRENCY",
  custom_title: "头衔卡",
  rename_card: "卡牌",
  makeup_card: "卡牌",
  rainbow_name: "卡牌",
  rainbow_id: "卡牌",
  avatar_frame: "装饰",
  animated_avatar: "装饰",
  vip: "VIP",
  app_vip: "VIP",
  ad_free: "特权",
  charity: "公益",
};

/** 后台详情 tab 键（资料全景 / 火花流水 / 登录记录 / 做种下载） */
export type DetailTab = "profile" | "spark" | "logins" | "seeding";
