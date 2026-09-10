import { api, paged } from "@/lib/api-client";
import type { TorrentListItem, UserPublic } from "@fluxtorrent/domain-types";

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
export async function getTopics(forumId: number): Promise<Topic[]> {
  try {
    return await api.get<Topic[]>(`/api/v1/forums/${forumId}/topics`);
  } catch {
    return [];
  }
}

export interface Post {
  id: number;
  username: string | null;
  body: string;
  created_at: string;
}
export interface TopicDetail {
  topic_id: number;
  title: string;
  forum_name: string | null;
  posts: Post[];
}
export async function getPosts(topicId: number): Promise<TopicDetail | null> {
  try {
    return await api.get<TopicDetail>(`/api/v1/forums/topics/${topicId}`);
  } catch {
    return null;
  }
}

export interface TopUser {
  rank: number;
  username: string;
  class_name: string;
  uploaded: number;
  downloaded: number;
  seed_size: number;
}
export async function getTopUsers(): Promise<TopUser[]> {
  try {
    return await api.get<TopUser[]>("/api/v1/top/users");
  } catch {
    return [];
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
export async function getPreserve(): Promise<PreserveEnvelope> {
  try {
    return await api.get<PreserveEnvelope>("/api/v1/preserve");
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

// ============ 缺口补齐：任务/银行/邀请/字幕/社交/投票（包子站同款入口） ============

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
