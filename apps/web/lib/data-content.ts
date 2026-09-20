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
      items: [] as PreserveItem[],
      total: 0,
      stats: {
        preserving: 0,
        continued: 0,
        official: 0,
        general: 0,
        today_in: 0,
        today_out: 0,
      } as PreserveStats,
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
