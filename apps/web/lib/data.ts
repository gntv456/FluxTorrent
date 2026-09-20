import { api, paged } from "@/lib/api-client";
import type { TorrentListItem, UserPublic } from "@fluxtorrent/domain-types";
import type { TagChipData } from "@/components/forum-bits";
import { DEFAULT_MEDAL_RARITIES, type MedalRarity } from "@/lib/medal-rarity";

export { formatBytes } from "./format";

// 前后端共享契约：实体 interface 已收编进 @fluxtorrent/domain-types（单一事实源）；
// 此处 import 供本文件函数签名使用，并 re-export 保持 web 内既有导入路径不变
import type {
  ShopItem,
  Medal,
  Forum,
  Topic,
  ForumTopics,
  SiteStats,
  MenuItem,
  Post,
  TopicDetail,
  FeedItem,
  FollowEntry,
  MyFollows,
  TopRow,
  TopBoards,
  Textbook,
  SeedRequest,
  PreserveItem,
  PreserveStats,
  PreserveEnvelope,
  Pool,
  TaskItem,
  BankDeposit,
  BankLoan,
  BankOverview,
  InviteItem,
  SubtitleItem,
  FriendItem,
  OfferItem,
  PollItem,
} from "@fluxtorrent/domain-types";
export type {
  ShopItem,
  Medal,
  Forum,
  Topic,
  ForumTopics,
  SiteStats,
  MenuItem,
  Post,
  TopicDetail,
  FeedItem,
  FollowEntry,
  MyFollows,
  TopRow,
  TopBoards,
  Textbook,
  SeedRequest,
  PreserveItem,
  PreserveStats,
  PreserveEnvelope,
  Pool,
  TaskItem,
  BankDeposit,
  BankLoan,
  BankOverview,
  InviteItem,
  SubtitleItem,
  FriendItem,
  OfferItem,
  PollItem,
};

// ============ 服务端数据获取（RSC 直连后端，§8.1 BFF 由 RSC 承担） ============

export async function getShopItems(): Promise<ShopItem[]> {
  try {
    return await api.get<ShopItem[]>("/api/v1/shop/items");
  } catch {
    return [];
  }
}

export async function getMedals(): Promise<Medal[]> {
  try {
    return await api.get<Medal[]>("/api/v1/medals");
  } catch {
    return [];
  }
}

/** 勋章稀有度词表（0143）：后台可维护；失败时回落内置兜底，页面不至于空掉 */
export async function getMedalRarities(): Promise<MedalRarity[]> {
  try {
    const rows = await api.get<MedalRarity[]>("/api/v1/medal-rarities");
    return rows.length > 0 ? rows : DEFAULT_MEDAL_RARITIES;
  } catch {
    return DEFAULT_MEDAL_RARITIES;
  }
}

export async function getForums(): Promise<Forum[]> {
  try {
    return await api.get<Forum[]>("/api/v1/forums");
  } catch {
    return [];
  }
}

export async function getSiteStats(): Promise<SiteStats | null> {
  try {
    return await api.get<SiteStats>("/api/v1/stats");
  } catch {
    return null;
  }
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

export async function getTopBoards(): Promise<TopBoards> {
  const empty: TopBoards = { bonus: [], uploaded: [], downloaded: [], seedtime: [], hourly: [], torrents: [] };
  try {
    return await api.get<TopBoards>("/api/v1/top/boards");
  } catch {
    return empty;
  }
}

export async function getTextbooks(): Promise<Textbook[]> {
  try {
    return await api.get<Textbook[]>("/api/v1/textbooks");
  } catch {
    return [];
  }
}

export async function getRequests(): Promise<SeedRequest[]> {
  try {
    return await api.get<SeedRequest[]>("/api/v1/requests");
  } catch {
    return [];
  }
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

export async function getTasks(): Promise<TaskItem[]> {
  try {
    return await api.get<TaskItem[]>("/api/v1/tasks");
  } catch {
    return [];
  }
}

export async function getBankDeposits(): Promise<BankDeposit[]> {
  try {
    return await api.get<BankDeposit[]>("/api/v1/bank/deposits");
  } catch {
    return [];
  }
}

export async function getInvites(): Promise<InviteItem[]> {
  try {
    return await api.get<InviteItem[]>("/api/v1/invites");
  } catch {
    return [];
  }
}

export async function getSubtitles(): Promise<SubtitleItem[]> {
  try {
    return await api.get<SubtitleItem[]>("/api/v1/subtitles");
  } catch {
    return [];
  }
}

export async function getFriends(): Promise<FriendItem[]> {
  try {
    return await api.get<FriendItem[]>("/api/v1/friends");
  } catch {
    return [];
  }
}

export async function getOffers(): Promise<OfferItem[]> {
  try {
    return await api.get<OfferItem[]>("/api/v1/offers");
  } catch {
    return [];
  }
}

export async function getPolls(): Promise<PollItem[]> {
  try {
    return await api.get<PollItem[]>("/api/v1/fun/polls");
  } catch {
    return [];
  }
}

export type { TorrentListItem, UserPublic };
