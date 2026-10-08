/**
 * 站点域面板·共享类型（从 components/staff-tools-site.tsx 按域拆出）：
 * 统计/清理结果/广告/无法连接用户/上传者/客户端/投票行类型。
 */

export interface SiteStats {
  users: number;
  torrents: number;
  seeding: number;
  leeching: number;
  comments: number;
  messages: number;
  redis: string;
  db: string;
  uptime_secs: number;
}

export interface CleanupResult {
  expired_promotions: number;
  expired_warnings: number;
  old_login_events: number;
  old_password_resets: number;
}

export interface AdItem {
  id: number;
  title: string;
  html: string;
  position: string;
  enabled: boolean;
  sort: number;
}

export interface NotConnectRow {
  id: number;
  username: string;
  torrents: number;
  last_seen_at: string | null;
}

/** 做种结论四档分布（/admin/conn/dist，0310）
 *  -1 未测 | -2 无法验证（加密客户端/白名单/CGNAT）| 0 不可信 | 1 可信
 *  通用站用 -2 与 0 的比例判断该不该收紧口径：-2 大 = 站内多加密客户端，
 *  此时把「无法验证」当作弊罚就是自伤做种供给。 */
export interface ConnDistRow {
  state: number;
  n: number;
  users: number;
}

export interface UploaderRow {
  id: number;
  username: string;
  uploads: number;
  seeding: number;
  total_size: number;
}

export interface AgentRow {
  agent: string;
  peers: number;
}

export interface PollRow {
  id: number;
  question: string;
  closed: boolean;
  votes: number;
  created_at: string;
}
