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
  /** IMDB id（0148：种子页字幕面板按此合并同片字幕；缺省 null） */
  imdb_id: string | null;
  owner_name: string | null;
  created_at: string;
  /** 行内标签徽标（0159 P1）：随行 json_agg，按字典 sort DESC, id 排序；旧数据缺省 */
  tags?: TagBadge[];
  /** 维度名列表（R3-三步）：行副题「学段 · 媒介 · 版本」由 sections 单源下发 */
  sec_names?: string[];
}

/** 列表行标签徽标（0159 P1：种子行回显——筛选与展示不再断裂） */
export interface TagBadge {
  id: number;
  name: string;
  kind: string;
  bg_color: string;
  color: string;
}

/** 种子评论（M07；0155 点赞 + 0156 嵌套回复） */
export interface TorrentComment {
  id: number;
  torrent_id: number;
  username: string | null;
  body: string;
  created_at: string;
  /** 点赞数 */
  likes?: number;
  /** 当前用户是否已赞 */
  liked_by_me?: boolean;
  /** 指向根评论 id；顶层评论为 null */
  parent_id?: number | null;
  /** 被回复人用户名（「回复 @xxx」显示） */
  reply_to_user?: string | null;
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

// ============ 模块注册表（U1 §4.3：与迁移 modules 表 / Rust modules.rs key::ALL 同步） ============
//
// 2026-09-25：原先这里手抄了一份 union 又手抄了一份数组，Rust 侧再加键时只加了数组项
// （invites 就是这么漏的）。改为**数组是唯一清单**、union 由 typeof 派生；
// 三份（Rust key::ALL / 本数组 / DB modules 表）由 scripts/module_keys_guard.mjs 比对。

/** 模块键唯一清单（顺序与 Rust key::ALL 一致；core 域不设开关） */
export const MODULE_KEYS = [
  // 社区
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
  "invites",
  // 经济
  "promo_buy",
  "bank",
  "shop",
  "magic_pool",
  "vouchers",
  "resurrections",
  "wishlist",
  // 娱乐
  "games",
  "farm",
  "gomoku",
  "contests",
  // 运营
  "attendance",
  "medals",
  "dressup",
  "jixiao",
  "tasks",
  "exams",
  "push",
] as const;

/** 可选模块键（派生自 MODULE_KEYS，不再手抄） */
export type ModuleKey = (typeof MODULE_KEYS)[number];


// ============ 站点实体契约（自 apps/web/lib/data.ts 收编，2026-09-20） ============

/** 论坛标签样式（0123）：词表与样式列全部来自 tag_dict（0063 带样式的通用标签字典）。
 *  样式列可能为空串（历史数据），空值回落主题默认。 */
export interface TagChipData {
  id: number;
  name: string;
  kind?: string;
  bg_color?: string;
  color?: string;
  font_size?: string;
  margin?: string;
  padding?: string;
  border_radius?: string;
}

export interface ShopItem {
  id: number;
  name: string;
  kind: string;
  price: number;
  /** 0207：stackable=可叠加数量；frame_id=头像框 SKU 绑定的框；effect=动态头像款式 */
  config?: {
    stackable?: boolean;
    frame_id?: number;
    effect?: string;
    slot?: string;
    [k: string]: unknown;
  };
}

export interface Medal {
  id: number;
  name: string;
  price: number | null;
  rarity: string | null;
  limited: boolean;
  owned: boolean;
  wearing: boolean;
  description: string | null;
  duration_days: number | null;
  get_type: number;
  sale_begin_at: string | null;
  sale_end_at: string | null;
  inventory: number | null;
  bonus_addition_factor: number;
  category_id: number;
  category_name: string | null;
  /** 勋章图片 URL（medals.asset_ref）；空则展示位回落 🏅 */
  asset_ref: string | null;
  /** per-勋章赠送手续费（基点；null = 回退全站 gift_tax_bp，0204） */
  gift_fee_bp?: number | null;
}

export interface Forum {
  id: number;
  name: string;
  descr: string | null;
  topics: number;
  posts: number;
  latest_topic?: string;
  latest_author?: string;
  latest_at?: string;
  /** 分区/节点（0115）：首页按分类分组 */
  category_id?: number | null;
  category_name?: string | null;
  can_write?: boolean;
  can_create?: boolean;
  can_mod?: boolean;
}

/** 论坛分区/节点（0115 建表；0154 起前台/后台均返回，且含空分区）。
 *  ⚠️ 后端一度用 Rust 元组返回，序列化成「数组的数组」→ 前端按字段取值恒 undefined。 */
export interface ForumCategory {
  id: number;
  name: string;
  sort: number;
  /** 前台可见性；false 时非 staff 看不到该分区及其版块 */
  visible: boolean;
  /** 当前视角下该分区可读的版块数（与 forums 列表同套谓词） */
  forums: number;
}

/** 版块精简项（`GET /forums/boards`）：只给「移动主题到…」下拉用。
 *
 *  与 `Forum` 的区别：单条 SQL 搞定、不含 `forum_access()` 逐版块判定，
 *  也不带 latest_* / posts。可读性与分区可见性的过滤口径与 `/forums` 一致。 */
export interface BoardBrief {
  id: number;
  name: string;
  category_name: string | null;
}

export interface Topic {
  id: number;
  forum_id: number;
  title: string;
  username: string | null;
  replies: number;
  views: number;
  last_post_at: string | null;
  sticky?: boolean;
  locked?: boolean;
  /** 精华帖（0099 NP digest 口径） */
  digest?: boolean;
  /** 帖子类型（0115）：normal|bounty|poll|lottery */
  topic_type?: string;
  /** 标签（0123）：tag_dict id + 名称 + 样式列（TagChip 直接渲染） */
  tags?: TagChipData[];
}

export interface ForumTopics {
  forum_id: number;
  /** 版块名（随列表返回，避免为拿标题再打一次 /forums） */
  forum_name?: string | null;
  can_write: boolean;
  can_create: boolean;
  can_mod: boolean;
  /** 当前排序（0116）：hot|new */
  sort?: string;
  /** 当前标签筛选（0123）：tag_dict id */
  tag?: number | null;
  topics: Topic[];
}

/** 站点运行统计（/stats 需登录；页脚展示用，未登录返回 null） */
export interface SiteStats {
  users: number;
  torrents: number;
  dead: number;
  seed_size: number;
}

/** 自定义菜单项（admin menu_items 公开接口；仅返回启用且当前等级可见项，按 sort, id 排序） */
export interface MenuItem {
  id: number;
  location: string;
  label: string;
  url: string;
  parent_id: number;
  target: string;
  min_class: number;
  sort: number;
  enabled: boolean;
}

export interface Post {
  id: number;
  username: string | null;
  user_id: number | null;
  body: string;
  created_at: string;
  edited_at: string | null;
  edited_by: number | null;
  /** 点赞数（0116） */
  likes?: number;
  /** 当前登录用户是否已赞（0116） */
  liked_by_me?: boolean;
  /** 打赏总额/次数（0127） */
  tips?: number;
  tip_count?: number;
  /** 作者公开信息（楼层左栏 2026-09-23）：头像 URL */
  avatar_url?: string | null;
  /** 等级名（user_classes.name） */
  class_name?: string | null;
  /** 入站时间（users.created_at，注册时间） */
  author_joined_at?: string | null;
  /** 佩戴中的勋章（含稀有度供染色） */
  worn_medals?: {
    name: string;
    asset_ref: string | null;
    rarity: string | null;
  }[] | null;
  /** 楼中楼（0163）：所属顶层楼 id（顶层楼自身为 NULL） */
  root_id?: number | null;
  /** 直接回复的目标楼层 id（顶层楼为 NULL） */
  parent_id?: number | null;
  /** 被回复楼层作者名（渲染「回复 @xxx」） */
  reply_to_name?: string | null;
  /** 该顶层楼的楼中楼（随主楼返回，时间正序；仅顶层楼非空） */
  replies?: Post[] | null;
}

export interface TopicDetail {
  topic_id: number;
  title: string;
  forum_id: number;
  forum_name: string | null;
  sticky: boolean;
  locked: boolean;
  /** 精华帖（0099 NP digest 口径） */
  digest?: boolean;
  /** 帖子类型（0115）：normal|bounty|poll|lottery */
  topic_type?: string;
  /** 悬赏（0124）：金额/状态/中选楼层 */
  bounty_spark?: number;
  bounty_status?: string;
  bounty_post_id?: number | null;
  /** 投票（0125）：topic_type=poll 时非空 */
  poll?: {
    options: string[];
    closed: boolean;
    my_vote: number | null;
    total: number;
    counts: { index: number; votes: number }[];
  } | null;
  /** 抽奖（0126）：topic_type=lottery 时非空 */
  lottery?: {
    winners: number;
    prize: number;
    ticket: number;
    status: string;
    draw_at: string;
    entries: number;
    joined: boolean;
    my_won: boolean;
    winner_ids: { id: number; name: string | null }[];
  } | null;
  /** 标签（0123）：头部 TagChip 渲染 */
  tags?: TagChipData[];
  is_op: boolean;
  current_user_id?: number | null;
  can_write: boolean;
  can_mod: boolean;
  posts: Post[];
  /** 收藏数（0116） */
  favorites?: number;
  /** 当前登录用户是否已收藏（0116） */
  faved?: boolean;
  /** 本窗口外还有更早楼层（长帖游标）：前端显示「加载更早的回复」 */
  has_more?: boolean;
}

/** 关注流条目（0121）：via 表示命中的关注来源 */
export interface FeedItem {
  topic_id: number;
  title: string;
  forum_id: number;
  forum_name: string | null;
  username: string | null;
  topic_type?: string;
  last_post_at: string | null;
  created_at: string;
  sticky?: boolean;
  locked?: boolean;
  replies: number;
  via: string;
}

export interface FollowEntry {
  id: number;
  name: string;
  forum_id?: number;
}

export interface MyFollows {
  users: FollowEntry[];
  forums: FollowEntry[];
  topics: FollowEntry[];
}

export interface TopRow {
  rank: number;
  username: string;
  class_name: string;
  title: string | null;
  avatar_url: string | null;
  avatar_frame_css?: string | null;
  avatar_frame_image?: string | null;
  val: number;
}

export interface TopBoards {
  bonus: TopRow[];
  uploaded: TopRow[];
  downloaded: TopRow[];
  seedtime: TopRow[];
  hourly: TopRow[];
  torrents: TopRow[];
}

export interface Textbook {
  id: number;
  subject: string;
  edition: string;
  grade: string;
  volume: string | null;
  publisher: string | null;
  downloads: number;
  torrent_id: number | null;
}

export interface SeedRequest {
  id: number;
  username: string | null;
  title: string;
  descr: string | null;
  bounty: number;
  status: number;
  fulfilled_torrent_id: number | null;
}

export interface PreserveItem {
  torrent_id: number;
  name: string;
  size: number;
  seeders: number;
  claimed_by: string | null;
  // 资源库行同构字段（保种区列表复用资源库行渲染）
  small_descr: string | null;
  category_id: number;
  medium_id: number | null;
  grade_id: number | null;
  edition_id: number | null;
  leechers: number;
  times_completed: number;
  comments: number;
  official: boolean;
  anonymous: boolean;
  sticky: boolean;
  promotion: string | null;
  promotion_ends_at: string | null;
  poster: string | null;
  owner_name: string | null;
  created_at: string;
  /** 行内标签徽标（0159 P1）：保种区行同构复用；该接口暂不返回 → 恒空 */
  tags?: TagBadge[];
  /** 维度名列表（R3-三步）：保种区接口暂不返回 → 恒空数组 */
  sec_names?: string[];
}

export interface PreserveStats {
  preserving: number;
  continued: number;
  official: number;
  general: number;
  today_in: number;
  today_out: number;
}

export interface PreserveEnvelope {
  items: PreserveItem[];
  total: number;
  stats: PreserveStats;
  page: number;
  per_page: number;
}

export interface Pool {
  month: string;
  donated: number;
  goal: number;
  progress: number;
  promo_started: boolean;
  top_donors: [string, number][];
}

export interface TaskItem {
  id: number;
  name: string;
  metric: Record<string, unknown>;
  reward: number;
  penalty: number;
  claim_limit: number | null;
  claimed: number;
  starts_at: string;
  ends_at: string;
}

export interface BankDeposit {
  id: number;
  amount: number;
  term_days: number;
  interest: number;
  paid_interest: number;
  settle_mode: "maturity" | "daily";
  status: number;
  maturity_at: string;
}

export interface BankLoan {
  id: number;
  amount: number;
  daily_rate_bp: number;
  term_days: number;
  remaining: number;
  accrued_interest: number;
  status: string;
  due_at: string;
}

/** 贷款历史行（含进行中；paid_at/status 供前端区分态） */
export interface BankLoanHistoryItem {
  id: number;
  amount: number;
  daily_rate_bp: number;
  term_days: number;
  remaining: number;
  accrued_interest: number;
  status: "active" | "defaulted" | "paid";
  due_at: string;
  paid_at: string | null;
  created_at: string;
}

/** 利息流水行（kind: demand 活期 / fixed 定期 / loan 贷款计提） */
export interface BankInterestRecord {
  id: number;
  kind: "demand" | "fixed" | "loan";
  reference_id: number;
  amount: number;
  rate_bp: number;
  calc_date: string;
}

export interface BankOverview {
  spark_balance: number;
  demand: { balance: number; daily_rate_bp: number };
  fixed: { active_total: number; active_count: number };
  loan: BankLoan | null;
  total_asset: number;
  net_asset: number;
  loan_outstanding: number;
  max_loan: number;
  limits: {
    min_deposit: number;
    max_deposit: number;
    min_demand: number;
    min_loan: number;
    penalty_bp: number;
  };
  site: {
    demand_total: number;
    demand_count: number;
    fixed_active_total: number;
    fixed_count: number;
    loan_outstanding_total: number;
    loan_count: number;
    today_interest_records: number;
    settle_healthy: boolean;
    settle_mode: "maturity" | "daily";
  };
  fixed_rates: { term_days: number; annual_rate: number }[];
  loan_rates: { term_days: number; daily_rate_bp: number }[];
}

export interface InviteItem {
  id: number;
  code: string;
  status: number;
  used_by: string | null;
  expires_at: string;
}

export interface SubtitleItem {
  id: number;
  torrent_id: number | null;
  username: string | null;
  title: string;
  lang: string | null;
  downloads: number;
  created_at: string;
}

export interface FriendItem {
  username: string;
  list: string;
}

export interface OfferItem {
  id: number;
  username: string | null;
  torrent_id: number | null;
  torrent_name: string | null;
  votes: number;
  promoted: boolean;
  created_at: string;
}

export interface PollItem {
  id: number;
  question: string;
  options: string[];
  closed: boolean;
  my_vote: number | null;
  total_votes: number;
  counts: { index: number; votes: number }[];
}
