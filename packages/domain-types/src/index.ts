/**
 * @fluxtorrent/domain-types
 * 前后端唯一共享契约（方案 §8.1 规则 5）：API 信封、错误码、分页、业务枚举。
 * 后端 Rust 侧的错误码/枚举必须与此处一一对应（CI openapi drift 校验）。
 */

// ============ 错误码（方案 §8.2 分段） ============

export const ErrorCode = {
  // 1xxx 通用
  INTERNAL: 1000,
  BAD_REQUEST: 1001,
  VALIDATION: 1002,
  NOT_FOUND: 1004,
  RATE_LIMITED: 1015,
  // 2xxx 认证
  UNAUTHORIZED: 2001,
  FORBIDDEN: 2003,
  INVALID_CREDENTIALS: 2004,
  INVITE_INVALID: 2005,
  INVITE_USED: 2006,
  USERNAME_TAKEN: 2007,
  MUST_RESET_PASSWORD: 2008,
  TWO_FACTOR_REQUIRED: 2010,
  TWO_FACTOR_INVALID: 2011,
  // 3xxx 种子
  TORRENT_NOT_FOUND: 3001,
  TORRENT_NOT_APPROVED: 3002,
  TORRENT_INVALID_FILE: 3003,
  TORRENT_DUPLICATE: 3004,
  // 4xxx 经济
  INSUFFICIENT_SPARK: 4001,
  LEDGER_CONFLICT: 4002,
  // 41xx 模块开关（U1 §5.1：本站未开放该功能，与权限 403 区分）
  MODULE_DISABLED: 4101,
  // 5xxx 社区
  COMMENT_NOT_FOUND: 5001,
  ALREADY_THANKED: 5002,
} as const;

export type ErrorCodeValue = (typeof ErrorCode)[keyof typeof ErrorCode];

// ============ 响应信封（方案 §8.2） ============

export interface ApiEnvelope<T> {
  code: number;
  message: string;
  data: T;
  request_id: string;
}

// ============ 游标分页（方案 §8.2） ============

export interface PageParams {
  cursor?: string;
  limit?: number; // ≤ 50，服务端钳制
}

export interface Page<T> {
  items: T[];
  next_cursor: string | null;
  total_estimate: number;
}

// ============ 业务枚举（与 PG 枚举/后端 serde 对齐） ============

/** 促销类型（M06，旧站 spstate 全集） */
export type PromotionKind =
  | "none"
  | "free"
  | "x2"
  | "x2free"
  | "half"
  | "x2half"
  | "p30";

/** 种子审批状态（M04） */
export const TorrentApproval = {
  PENDING: 0,
  APPROVED: 1,
  REJECTED: 2,
} as const;

/** 火花流水类型（M11，spark_ledger.kind） */
export type SparkLedgerKind =
  | "migration_initial"
  | "seeding_reward"
  | "upload"
  | "subtitle"
  | "forum"
  | "vote"
  | "promote"
  | "shop"
  | "bank_deposit"
  | "bank_withdraw"
  | "attendance"
  | "pool_donate"
  | "task_reward"
  | "task_penalty"
  | "game"
  | "admin_grant"
  | "refund";

/** 分类（M02 五行筛选第一行；旧站 cat401-410 迁移） */
export const CATEGORIES = [
  "preschool",
  "primary",
  "junior",
  "vocational",
  "senior",
  "edu_media",
  "documentary",
] as const;
export type Category = (typeof CATEGORIES)[number];

/** 媒介（M02 第二行） */
export const MEDIA = [
  "video",
  "audio",
  "book",
  "document",
  "notes",
  "courseware",
  "software",
  "image",
] as const;
export type Medium = (typeof MEDIA)[number];

/** 官种 tag（旧站口径 tag_id=3） */
export const OFFICIAL_TAG_ID = 3;

/** 关键魔法数字（方案 §8.5-8：口径常量化） */
export const LIMITS = {
  MAX_PAGE_LIMIT: 50,
  INVITE_TTL_HOURS: 72,
  JWT_TTL_HOURS: 24,
  ANNOUNCE_INTERVAL_SECS: 300,
  PEER_TIMEOUT_SECS: 60,
  /** 保种区移出阈值（旧站口径：做种 > 7） */
  PRESERVE_EXIT_SEEDERS: 7,
  /** 保种移出后免费延续天数（旧站口径：3 天） */
  PRESERVE_GRACE_DAYS: 3,
  /** 站免池月目标（旧站口径：200 万火花） */
  MAGIC_POOL_GOAL: 2_000_000,
} as const;

// ============ 核心视图模型 ============

export interface TorrentListItem {
  id: number;
  info_hash: string;
  name: string;
  small_descr: string | null;
  category_id: number;
  /** 介质列（0087 起可空，新数据在 torrent_sections） */
  medium_id: number | null;
  grade_id: number | null;
  edition_id: number | null;
  size: number;
  seeders: number;
  leechers: number;
  times_completed: number;
  comments: number;
  official: boolean;
  anonymous: boolean;
  approval_status: number;
  sticky: boolean;
  promotion: PromotionKind | null;
  /** 进行中促销的截止时刻（ISO；列表展示「剩余时间」，好学站口径） */
  promotion_ends_at: string | null;
  /** 媒体评分（media_info.rating；首页海报墙展示，缺省 null） */
  rating: string | null;
  /** 海报图 URL（media_info.poster；缺省 null 时前端用生成式海报兜底） */
  poster: string | null;
  owner_name: string | null;
  created_at: string;
}

/** 种子评论（M07） */
export interface TorrentComment {
  id: number;
  torrent_id: number;
  username: string | null;
  body: string;
  created_at: string;
}

export interface UserPublic {
  id: number;
  username: string;
  class_name: string;
  title: string | null;
  avatar_url: string | null;
  uploaded: number;
  downloaded: number;
  created_at: string;
}

// ============ 模块注册表（U1 §4.3：与迁移 0107 modules 表 / Rust modules.rs key::ALL 三方同步） ============

/** 可选模块键（core 域不设开关；/site-profile 返回 modules: Record<ModuleKey, boolean>） */
export type ModuleKey =
  // 社区
  | "textbooks"
  | "showcase"
  | "social"
  | "forums"
  | "messages"
  | "friends"
  | "offers"
  | "requests"
  | "subtitles"
  | "preserve"
  | "shoutbox"
  // 经济
  | "promo_buy"
  | "bank"
  | "shop"
  | "magic_pool"
  | "vouchers"
  | "resurrections"
  | "wishlist"
  // 娱乐
  | "games"
  | "farm"
  | "gomoku"
  | "contests"
  // 运营
  | "attendance"
  | "medals"
  | "dressup"
  | "jixiao"
  | "tasks"
  | "exams"
  | "push";

/** 模块开关联合键集合（契约测试用：长度与 Rust key::ALL / 迁移行数一致 = 29） */
export const MODULE_KEYS: readonly ModuleKey[] = [
  "textbooks",
  "showcase",
  "social",
  "forums",
  "messages",
  "friends",
  "offers",
  "requests",
  "subtitles",
  "preserve",
  "shoutbox",
  "promo_buy",
  "bank",
  "shop",
  "magic_pool",
  "vouchers",
  "resurrections",
  "wishlist",
  "games",
  "farm",
  "gomoku",
  "contests",
  "attendance",
  "medals",
  "dressup",
  "jixiao",
  "tasks",
  "exams",
  "push",
] as const;
