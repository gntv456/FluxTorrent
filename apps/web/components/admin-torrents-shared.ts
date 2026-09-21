/**
 * 后台种子管理共享类型与常量（从 components/admin-torrents.tsx 按域拆出）：
 * 行类型（种子/拒绝原因/操作记录/记录/标签/分类）、审批与促销标签、字节格式化。
 */

export interface AdminTorrentRow {
  id: number;
  name: string;
  owner_id: number | null;
  owner_name: string | null;
  category_id: number;
  size: number;
  seeders: number;
  leechers: number;
  approval_status: number;
  deny_reason: string | null;
  deny_note: string | null;
  sticky: boolean;
  pos_state: number;
  pos_state_until: string | null;
  pick_type: number;
  promotion: string | null;
  promotion_ends_at: string | null;
  hr: boolean;
  created_at: string;
}

export interface DenyReason {
  id: number;
  sort: number;
  reason: string;
  enabled: boolean;
}

export interface TorrentOpRow {
  id: number;
  torrent_id: number;
  torrent_name: string | null;
  operator_name: string | null;
  action: string;
  detail: Record<string, unknown> | null;
  created_at: string;
}

export interface RecordRow {
  id: number;
  username: string;
  [k: string]: unknown;
}

export interface TagRow {
  id: number;
  name: string;
  kind: string;
  enabled: boolean;
}
export interface CatRow {
  id: number;
  name: string;
}

export interface LoginRow {
  id: number;
  username: string;
  ip: string;
  ok: boolean;
  country: string | null;
  country_name: string | null;
  city: string | null;
  created_at: string;
}

export const APPROVAL = ["待审", "通过", "拒绝", "已删除"];
export const PROMO_LABEL: Record<string, string> = {
  free: "免费",
  x2: "双倍",
  x2free: "2xFree",
  half: "半价",
  x2half: "2x半价",
  p30: "30%",
};

export function fmtBytes(n: number): string {
  if (n >= 1099511627776) return `${(n / 1099511627776).toFixed(2)} TB`;
  if (n >= 1073741824) return `${(n / 1073741824).toFixed(2)} GB`;
  if (n >= 1048576) return `${(n / 1048576).toFixed(2)} MB`;
  return `${(n / 1024).toFixed(2)} KB`;
}

export type SubTab = "torrents" | "deny" | "ops" | "spark" | "buys" | "logins";

/** 批量动作 → 中文名（批量工具条确认文案用） */
export function actionLabel(a: string): string {
  return (
    {
      sticky: "置顶",
      promo: "设置促销",
      recommend: "推荐",
      set_tags: "打标",
      clear_tags: "清除标签",
      hr: "标记H&R",
      unhr: "取消H&R",
      change_category: "改分类",
      delete: "删除",
    }[a] ?? a
  );
}
