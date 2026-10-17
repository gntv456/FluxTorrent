/** 服务端数据获取（RSC 直连后端，§8.1 BFF 由 RSC 承担）：
 *  资源/保种/求种/魔力池/任务/银行/邀请/字幕/社交/投票。
 *  从 lib/data.ts 按域拆出；data.ts re-export 保持既有导入路径不变。 */

import { api, paged } from "@/lib/api-client";
import type { TorrentListItem } from "@fluxtorrent/domain-types";
import type {
  Textbook,
  SeedRequest,
  PreserveItem,
  PreserveStats,
  PreserveEnvelope,
  Pool,
  TaskItem,
  BankDeposit,
  InviteItem,
  SubtitleItem,
  FriendItem,
  OfferItem,
  PollItem,
} from "@fluxtorrent/domain-types";

export async function getTextbooks(): Promise<Textbook[]> {
  return await api.get<Textbook[]>("/api/v1/textbooks");
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
  // P-286（10-14 实测）：这里原来 catch 后返回「items 空 + stats 全 0」，
  // 后端不可达时保种区照样渲染成「共 0 个结果 · 暂无保种需求」——
  // 把失败读成「这里没有要救的种」，而这个板块正是催会员行动的。
  // 改为直接抛出，由页面按四态渲染错误面板（同 collections/torrents）。
  const qs = new URLSearchParams();
  for (const [k, v] of Object.entries(params ?? {})) {
    if (v) qs.set(k, v);
  }
  const suffix = qs.toString() ? `?${qs.toString()}` : "";
  return await api.get<PreserveEnvelope>(`/api/v1/preserve${suffix}`);
}

export async function getPool(): Promise<Pool | null> {
  return await api.get<Pool>("/api/v1/magic-pool");
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
  return await api.get<FriendItem[]>("/api/v1/friends");
}

export async function getOffers(): Promise<OfferItem[]> {
  return await api.get<OfferItem[]>("/api/v1/offers");
}

export async function getPolls(): Promise<PollItem[]> {
  try {
    return await api.get<PollItem[]>("/api/v1/fun/polls");
  } catch {
    return [];
  }
}

/** GET /me/bookmarks 的行（misc_handlers.rs）：后端按 limit 截断且没有
 *  cursor，默认 50、上限 100 —— 取满要说「仅显示最近 100 条」。 */
export interface BookmarkRow {
  torrent_id: number;
  name: string;
  small_descr: string | null;
  size: number;
  seeders: number;
  leechers: number;
  created_at: string;
}

/** 我的收藏。返回 null = 读失败（不能演成「还没收藏任何种子」）。 */
export async function getMyBookmarks(
  limit = 100,
): Promise<BookmarkRow[] | null> {
  try {
    const per = Math.min(100, Math.max(1, limit));
    return await api.get<BookmarkRow[]>(
      `/api/v1/me/bookmarks?limit=${per}`,
    );
  } catch {
    return null;
  }
}

/** 封禁公示的一行（GET /ban-log）。`name_masked` 是后端打码后的名字——
 *  接口本身就不返回原名，前端拿不到、也就无从漏出去。 */
export interface BanLogRow {
  uid: number;
  name_masked: string;
  /** 1=禁言 2=封号 */
  status: number;
  reason: string;
  acted_at: string | null;
  warned_until: string | null;
  cheat_banned_until: string | null;
}

export interface BanLogPage {
  rows: BanLogRow[];
  total: number;
  page: number;
  per: number;
}

/** 封禁公示名单。失败回 null——「没取到」和「本站没有封禁记录」是两句话。 */
export async function getBanLog(o: {
  q?: string;
  kind?: string;
  page: number;
  per?: number;
}): Promise<BanLogPage | null> {
  try {
    const sp = new URLSearchParams();
    if (o.q) sp.set("q", o.q);
    if (o.kind) sp.set("kind", o.kind);
    sp.set("page", String(o.page));
    if (o.per) sp.set("per", String(o.per));
    return await api.get<BanLogPage>(`/api/v1/ban-log?${sp.toString()}`);
  } catch {
    return null;
  }
}

/** 已收录的友情链接（GET /links，只出审核通过的）。 */
export interface PublicLink {
  name: string;
  url: string;
  title: string | null;
}

export async function getPublicLinks(): Promise<PublicLink[] | null> {
  try {
    return await api.get<PublicLink[]>("/api/v1/links");
  } catch {
    return null;
  }
}

