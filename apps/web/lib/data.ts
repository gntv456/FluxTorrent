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
export async function getPosts(topicId: number): Promise<Post[]> {
  try {
    return await api.get<Post[]>(`/api/v1/forums/topics/${topicId}`);
  } catch {
    return [];
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
export async function getPreserve(): Promise<PreserveItem[]> {
  try {
    return await api.get<PreserveItem[]>("/api/v1/preserve");
  } catch {
    return [];
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

export type { TorrentListItem, UserPublic };
