"use client";

/**
 * 管理后台共享类型与工具（从 app/(main)/admin/page.tsx 按域拆出）：
 * 概览 / 待审种子 / 申诉 / 审计 / 统计 / 作弊行 / 促销行 + 旧 URL 工具名映射。
 */

export interface Overview {
  pending_reviews: number;
  open_reports: number;
  users: number;
  torrents: number;
  banned_users: number;
  /** E16 大盘三块（空库各值为 0/[]） */
  trend?: [string, number, number][];
  health?: {
    alive: number;
    dead: number;
    alive_rate: number;
    avg_seeders: number;
  };
  wau?: number;
}

export interface PendingTorrent {
  id: number;
  name: string;
  owner_id: number | null;
  size: number;
  created_at: string;
  /** 0285 补：审核队列内容面（旧版只有上面 5 个字段，版主看不到内容就要判生死） */
  owner_name?: string | null;
  category_name?: string | null;
  small_descr?: string;
  descr_excerpt?: string;
  numfiles?: number;
  screenshots?: number;
  has_nfo?: boolean;
  has_media_info?: boolean;
  anonymous?: boolean;
  official_tag?: boolean;
  price?: number;
  dup_hash?: number;
  dup_name?: number;
  owner_approved?: number;
  owner_denied?: number;
}

export interface AppealRow {
  id: number;
  username: string;
  kind: string;
  ref_id: number | null;
  body: string;
  status: string;
  result_note: string | null;
  created_at: string;
}

export interface AuditRow {
  id: number;
  actor_id: number | null;
  action: string;
  created_at: string;
}

export interface StatsData {
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

export interface CheaterRow {
  user_id: number;
  username: string;
  torrent_id: number | null;
  name: string | null;
  upspeed: number;
  uploaded_delta: number;
  announced_at: string;
}

export interface PromoRow {
  id: number;
  scope: string;
  kind: string;
  category_id: number | null;
  category_name: string | null;
  starts_at: string;
  ends_at: string;
}

/** 旧 URL 兼容：历史 ?tool= 值与现 tab_key 命名不一致，映射后旧书签不失效 */
export const LEGACY_TOOL: Record<string, string> = {
  reset: "resetpass",
  deletedisabled: "deldisabled",
  bannedemails: "emailbans",
  allowedemails: "emailbans",
  docleanup: "cleanup",
  admanage: "ads",
  allagents: "agents",
  polloverview: "polls",
  amountbonus: "incrementbulk",
  bonus: "incrementbulk",
  amountupload: "incrementbulk",
  upload: "incrementbulk",
  notconnectable: "notconnect",
  location: "locations",
  faqmanage: "faq",
  modrules: "rules",
  catmanage: "cats",
  mysql_stats: "dbstats",
  bitbucketlog: "syslog",
  // 邮件群发的 DB url 曾写作 ?tool=massmail，与 tab_key `mail` 不一致（0156 已改）。
  // 这里兜住仍然存在的老书签：没有这条会落进 panelEmpty 空面板。
  massmail: "mail",
  // 促销公告（promo）面板早被 FreeleechPanel 取代（同打 /admin/freeleech），
  // 旧入口指向「免费/促销状态」。
  promo: "freeleech",
};
