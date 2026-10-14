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
