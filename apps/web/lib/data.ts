import { api, paged } from "@/lib/api-client";
import type { TorrentListItem, UserPublic } from "@fluxtorrent/domain-types";
import type { TagChipData } from "@/components/forum-bits";

export { formatBytes } from "./format";

// ============ 服务端数据获取（RSC 直连后端，§8.1 BFF 由 RSC 承担） ============

export interface ShopItem {
  id: number;
  name: string;
  kind: string;
  price: number;
}
export async function getShopItems(): Promise<ShopItem[]> {
  try {
    return await api.get<ShopItem[]>("/api/v1/shop/items");
  } catch {
    return [];
  }
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
}
export async function getMedals(): Promise<Medal[]> {
  try {
    return await api.get<Medal[]>("/api/v1/medals");
  } catch {
    return [];
  }
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
export async function getForums(): Promise<Forum[]> {
  try {
    return await api.get<Forum[]>("/api/v1/forums");
  } catch {
    return [];
  }
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
export async function getSiteStats(): Promise<SiteStats | null> {
  try {
    return await api.get<SiteStats>("/api/v1/stats");
  } catch {
    return null;
  }
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
export async function getMenuItems(location: string): Promise<MenuItem[]> {
  try {
    return await api.get<MenuItem[]>(
      `/api/v1/menu-items?location=${encodeURIComponent(location)}`,
    );
  } catch {
    return [];
  }
}
export async function getTopics(
  forumId: number,
  sort?: string,
  tag?: number,
): Promise<ForumTopics | null> {
  try {
    const qs = new URLSearchParams();
    if (sort) qs.set("sort", sort);
    if (tag) qs.set("tag", String(tag));
    const suffix = qs.toString() ? `?${qs.toString()}` : "";
    return await api.get<ForumTopics>(`/api/v1/forums/${forumId}/topics${suffix}`);
  } catch {
    return null;
  }
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
export async function getPosts(
  topicId: number,
  before?: number,
): Promise<TopicDetail | null> {
  try {
    const qs = before ? `?before=${before}` : "";
    return await api.get<TopicDetail>(`/api/v1/forums/topics/${topicId}${qs}`);
  } catch {
    return null;
  }
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
/** 关注流分页游标（0123）：next_before 为空表示没有下一页 */
export async function getFeed(
  limit = 50,
  before?: string,
): Promise<{ items: FeedItem[]; next_before: string | null }> {
  try {
    const qs = before ? `?limit=${limit}&before=${encodeURIComponent(before)}` : `?limit=${limit}`;
    const r = await api.get<{ items: FeedItem[]; next_before: string | null }>(
      `/api/v1/forums/feed${qs}`,
    );
    return { items: r?.items ?? [], next_before: r?.next_before ?? null };
  } catch {
    return { items: [], next_before: null };
  }
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
export async function getMyFollows(): Promise<MyFollows> {
  const empty: MyFollows = { users: [], forums: [], topics: [] };
  try {
    return (await api.get<MyFollows>("/api/v1/follows/mine")) ?? empty;
  } catch {
    return empty;
  }
}

/** 论坛标签字典（0123）：公开接口，含 tag_dict 样式列（TagChip 渲染源） */
export async function getForumTags(): Promise<TagChipData[]> {
  try {
    return (await api.get<TagChipData[]>("/api/v1/forums/tags")) ?? [];
  } catch {
    return [];
  }
}

export interface TopRow {
  rank: number;
  username: string;
  class_name: string;
  title: string | null;
  avatar_url: string | null;
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
export async function getTopBoards(): Promise<TopBoards> {
  const empty: TopBoards = { bonus: [], uploaded: [], downloaded: [], seedtime: [], hourly: [], torrents: [] };
  try {
    return await api.get<TopBoards>("/api/v1/top/boards");
  } catch {
    return empty;
  }
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
export async function getTextbooks(): Promise<Textbook[]> {
  try {
    return await api.get<Textbook[]>("/api/v1/textbooks");
  } catch {
    return [];
  }
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
export async function getRequests(): Promise<SeedRequest[]> {
  try {
    return await api.get<SeedRequest[]>("/api/v1/requests");
  } catch {
    return [];
  }
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
export async function getPreserve(
  params?: Record<string, string | undefined>,
): Promise<PreserveEnvelope> {
  try {
    const qs = new URLSearchParams();
    for (const [k, v] of Object.entries(params ?? {})) {
      if (v) qs.set(k, v);
    }
    const suffix = qs.toString() ? `?${qs.toString()}` : "";
    return await api.get<PreserveEnvelope>(`/api/v1/preserve${suffix}`);
  } catch {
    return {
      items: [],
      total: 0,
      stats: { preserving: 0, continued: 0, official: 0, general: 0, today_in: 0, today_out: 0 },
      page: 0,
      per_page: 50,
    };
  }
}

export interface Pool {
  month: string;
  donated: number;
  goal: number;
  progress: number;
  promo_started: boolean;
  top_donors: [string, number][];
}
export async function getPool(): Promise<Pool | null> {
  try {
    return await api.get<Pool>("/api/v1/magic-pool");
  } catch {
    return null;
  }
}

export async function getTorrents(
  params: Record<string, string | number | boolean | undefined>,
): Promise<TorrentListItem[]> {
  try {
    const page = await paged<TorrentListItem>("/api/v1/torrents", params);
    return page.items;
  } catch {
    return [];
  }
}

// ============ 缺口补齐：任务/银行/邀请/字幕/社交/投票（参考站同款入口） ============

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
export async function getTasks(): Promise<TaskItem[]> {
  try {
    return await api.get<TaskItem[]>("/api/v1/tasks");
  } catch {
    return [];
  }
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
export async function getBankDeposits(): Promise<BankDeposit[]> {
  try {
    return await api.get<BankDeposit[]>("/api/v1/bank/deposits");
  } catch {
    return [];
  }
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
export async function getInvites(): Promise<InviteItem[]> {
  try {
    return await api.get<InviteItem[]>("/api/v1/invites");
  } catch {
    return [];
  }
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
export async function getSubtitles(): Promise<SubtitleItem[]> {
  try {
    return await api.get<SubtitleItem[]>("/api/v1/subtitles");
  } catch {
    return [];
  }
}

export interface FriendItem {
  username: string;
  list: string;
}
export async function getFriends(): Promise<FriendItem[]> {
  try {
    return await api.get<FriendItem[]>("/api/v1/friends");
  } catch {
    return [];
  }
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
export async function getOffers(): Promise<OfferItem[]> {
  try {
    return await api.get<OfferItem[]>("/api/v1/offers");
  } catch {
    return [];
  }
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
export async function getPolls(): Promise<PollItem[]> {
  try {
    return await api.get<PollItem[]>("/api/v1/fun/polls");
  } catch {
    return [];
  }
}


export type { TorrentListItem, UserPublic };
