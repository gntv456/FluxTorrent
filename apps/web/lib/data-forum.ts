/** 服务端数据获取（RSC 直连后端，§8.1 BFF 由 RSC 承担）：
 *  论坛域（版面/话题/帖子/关注流/标签/排行）。
 *  从 lib/data.ts 按域拆出；data.ts re-export 保持既有导入路径不变。 */

import { api } from "@/lib/api-client";
import type { TagChipData } from "@/components/forum-bits";
import type {
  Forum,
  ForumCategory,
  ForumTopics,
  SiteStats,
  TopicDetail,
  FeedItem,
  MyFollows,
  TopBoards,
  BoardBrief,
} from "@fluxtorrent/domain-types";

/** 论坛索引：分区列表 + 版块列表。
 *  分区**含空分区**，前台据此渲染分组与空态占位——
 *  旧版从版块列表反推分组，导致新建的空分区在前台整块消失。 */
export async function getForumIndex(): Promise<{
  categories: ForumCategory[];
  forums: Forum[];
}> {
  try {
    const r = await api.get<{
      categories: ForumCategory[];
      forums: Forum[];
    }>("/api/v1/forums");
    return { categories: r?.categories ?? [], forums: r?.forums ?? [] };
  } catch {
    return { categories: [], forums: [] };
  }
}

export async function getForums(): Promise<Forum[]> {
  return (await getForumIndex()).forums;
}

/** 全站可读版块精简列表（「移动主题到…」下拉的数据源）。
 *
 *  比 `getForums()` 合适：后端单条 SQL，不走 `forum_access()` 逐版块判定
 *  （`/forums` 是 1+N 次查询），也不返回 latest 系与 posts 这些下拉用不到的字段。 */
export async function getBoards(): Promise<BoardBrief[]> {
  try {
    return (await api.get<BoardBrief[]>("/api/v1/forums/boards")) ?? [];
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
/** 视频内嵌白名单规则（0189）：渲染层据此生成播放器 src（二次校验在前端组件）。 */
export interface EmbedRule {
  id: number;
  provider: string;
  name_zh: string;
  url_pattern: string;
  embed_template: string;
  embed_origin: string;
  render_kind: "iframe" | "video";
  aspect: "16:9" | "4:3" | "1:1";
  extra_params: string | null;
}

/** 论坛视频 embed 规则（总开关关闭/失败 → 空数组，视频语法全部降级为链接）。 */
export async function getEmbedRules(): Promise<EmbedRule[]> {
  try {
    const r = await api.get<EmbedRule[]>("/api/v1/forums/embed-rules");
    return r ?? [];
  } catch {
    return [];
  }
}

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
