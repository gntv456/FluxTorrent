/** 服务端数据获取（RSC 直连后端，§8.1 BFF 由 RSC 承担）：
 *  论坛域（版面/话题/帖子/关注流/标签/排行）。
 *  从 lib/data.ts 按域拆出；data.ts re-export 保持既有导入路径不变。 */

import { api } from "@/lib/api-client";
import type { TagChipData } from "@/components/forum-bits";
import type {
  Forum,
  ForumCategory,
  ForumMine,
  ForumRecent,
  ForumSide,
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

/** 论坛首页右栏聚合：统计 / 今日热帖 / 最新回复。
 *  取档失败回 null——右栏整列不渲染，不拿空数组演成「今天没人发帖」。 */
export async function getForumSide(): Promise<ForumSide | null> {
  try {
    return await api.get<ForumSide>("/api/v1/forums/side");
  } catch {
    return null;
  }
}

/** 论坛首页左列用户卡（样图集 § forum 左列第一块）的视图模型。 */
export interface ForumUserCard {
  username: string;
  class_name: string | null;
  avatar_url: string | null;
  uploaded: number;
  downloaded: number;
  seeding: number;
  /** null = 余额取档失败（界面显 "—"，不演成 0） */
  spark: number | null;
}

/** 用户卡取数：`/me` 不回魔力（余额在 `/me/spark`，与商店余额条同口径），
 *  故两趟；魔力那条失败只降级为 `spark: null`，不带崩整张卡。
 *  未登录 / `/me` 失败回 null——用户卡整块不渲染，而不是演成
 *  「0.00 分享率 · 0 做种」的游客假数据。 */
export async function getForumUserCard(): Promise<ForumUserCard | null> {
  try {
    const [me, sparkRow] = await Promise.all([
      api.get<{
        username: string;
        class_name?: string | null;
        avatar_url?: string | null;
        uploaded: number;
        downloaded: number;
        seeding: number;
      }>("/api/v1/me"),
      api
        .get<{ balance?: number }>("/api/v1/me/spark")
        .catch(() => ({ balance: undefined })),
    ]);
    return {
      username: me.username,
      class_name: me.class_name ?? null,
      avatar_url: me.avatar_url ?? null,
      uploaded: me.uploaded,
      downloaded: me.downloaded,
      seeding: me.seeding,
      spark:
        typeof sparkRow?.balance === "number" ? sparkRow.balance : null,
    };
  } catch {
    return null;
  }
}

/** 我参与的论坛活动（关注流「我参与的 / 我的回复」两个标签的数据源）。
 *  取档失败回 null——界面按「没取到」说，不演成「你还没发过帖」。 */
export async function getMyForumActivity(
  type: "topics" | "replies",
  page: number,
): Promise<ForumMine | null> {
  try {
    return await api.get<ForumMine>(
      `/api/v1/forums/mine?type=${type}&page=${page}`,
    );
  } catch {
    return null;
  }
}

/** 论坛首页中栏话题流（样图集 § tplForum）：全站可读版块的最新主题。
 *  取档失败回 null——中栏按「没取到」说，不演成「今天没人发帖」。
 *  type：sticky=只看置顶 / digest=只看精华；缺省全量。 */
export async function getForumRecent(
  type?: string,
): Promise<ForumRecent | null> {
  try {
    const qs = type ? `?type=${encodeURIComponent(type)}` : "";
    return await api.get<ForumRecent>(`/api/v1/forums/recent${qs}`);
  } catch {
    return null;
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
  page?: number,
  type?: string,
): Promise<ForumTopics | null> {
  try {
    const qs = new URLSearchParams();
    if (sort) qs.set("sort", sort);
    if (tag) qs.set("tag", String(tag));
    if (type) qs.set("type", type);
    if (page && page > 1) qs.set("page", String(page));
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
): Promise<{ items: FeedItem[]; next_before: string | null } | null> {
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
    // 回 null 而不是空数组：关注流取档失败时，「你关注的没动静」是句假话
    return null;
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
  // P-309：这一层不再 catch 成「六榜全空」。页面 app/(main)/top/page.tsx 按
  // W-27 自己 try/catch 来区分「接口挂」与「真的全空」，但异常在这里就被吞掉，
  // 页面的 ErrorPanel 永远走不到——后端故障会被演成六榜「暂无数据」，而那正是
  // 页面注释声称已经修掉的形状。修法只能落在吞异常的这一层。
  return await api.get<TopBoards>("/api/v1/top/boards");
}
