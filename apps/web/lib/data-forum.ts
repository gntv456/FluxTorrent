/** 服务端数据获取（RSC 直连后端，§8.1 BFF 由 RSC 承担）：
 *  论坛域（版面/话题/帖子/关注流/标签/排行）。
 *  从 lib/data.ts 按域拆出；data.ts re-export 保持既有导入路径不变。 */

import { api } from "@/lib/api-client";
import type { TagChipData } from "@/components/forum-bits";
import type {
  Forum,
  ForumTopics,
  SiteStats,
  TopicDetail,
  FeedItem,
  MyFollows,
  TopBoards,
} from "@fluxtorrent/domain-types";

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
    return await api.get<ForumTopics>(
      `/api/v1/forums/${forumId}/topics${suffix}`,
    );
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
    return await api.get<TopicDetail>(
      `/api/v1/forums/topics/${topicId}${qs}`,
    );
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
    const qs = before
      ? `?limit=${limit}&before=${encodeURIComponent(before)}`
      : `?limit=${limit}`;
    const r = await api.get<{
      items: FeedItem[];
      next_before: string | null;
    }>(`/api/v1/forums/feed${qs}`);
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
  const empty: TopBoards = {
    bonus: [],
    uploaded: [],
    downloaded: [],
    seedtime: [],
    hourly: [],
    torrents: [],
  };
  try {
    return await api.get<TopBoards>("/api/v1/top/boards");
  } catch {
    return empty;
  }
}
