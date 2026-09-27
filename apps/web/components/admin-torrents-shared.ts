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
  /** 自建维度取值（G5 后台列表「维度」列，与前台同一形状） */
  sec_names: string[];
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
  /** 父分类（0188 层级）：显示走 catPath 拼「父 › 子」 */
  parent_id?: number | null;
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

/** 审批状态显示名（按 approval_status 作下标）；三语在 i18n `adminTorrents.approval` */
export const approvalList = (labels?: string[]): string[] => labels ?? [];

export const promoLabels = (
  labels?: Record<string, string>,
): Record<string, string> => labels ?? {};

/** 批量动作 → 显示名（批量工具条确认文案用）；三语在 `adminTorrents.actions` */
export function actionLabel(a: string, labels?: Record<string, string>) {
  return labels?.[a] ?? a;
}

export function fmtBytes(n: number): string {
  if (n >= 1099511627776) return `${(n / 1099511627776).toFixed(2)} TB`;
  if (n >= 1073741824) return `${(n / 1073741824).toFixed(2)} GB`;
  if (n >= 1048576) return `${(n / 1048576).toFixed(2)} MB`;
  return `${(n / 1024).toFixed(2)} KB`;
}

export type SubTab = "torrents" | "deny" | "ops" | "spark" | "buys" | "logins";
