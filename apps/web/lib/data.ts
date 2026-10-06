import { api } from "@/lib/api-client";
import type { TorrentListItem, UserPublic } from "@fluxtorrent/domain-types";
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
  BankLoanHistoryItem,
  BankInterestRecord,
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
  BankLoanHistoryItem,
  BankInterestRecord,
  BankOverview,
  InviteItem,
  SubtitleItem,
  FriendItem,
  OfferItem,
  PollItem,
};

// 数据装载函数已按域拆出（data-forum / data-content）；此处 re-export
// 保持既有 `@/lib/data` 导入路径不变（importer 零改动）。
export * from "./data-forum";
export * from "./data-content";

// ============ 本域：站点/商店/勋章/导航菜单 ============

/** 当前登录用户的魔力余额（商店余额条用）。未登录或取数失败返回 null——
 *  商店在匿名/异常下照常可用，只是不做「余额不足」预判（交后端校验）。
 *  口径修正：GET /me 从未返回 spark_balance（余额在 /me/spark），
 *  此前端余额条永远显示 "—"、差额预判从不生效。 */
export async function getMySpark(): Promise<number | null> {
  try {
    const s = await api.get<{ balance?: number }>("/api/v1/me/spark");
    return typeof s.balance === "number" ? s.balance : null;
  } catch {
    return null;
  }
}

export async function getShopItems(): Promise<ShopItem[]> {
  try {
    return await api.get<ShopItem[]>("/api/v1/shop/items");
  } catch {
    return [];
  }
}

/** GET /me/orders 响应（商城订单中心 P2-1） */
export interface MyOrdersEnvelope {
  rows: {
    id: number;
    item_id: number;
    item_name: string | null;
    kind: string;
    price: number;
    effect_applied: boolean;
    created_at: string;
  }[];
  total: number;
  page: number;
  per_page: number;
}

export async function getMyOrders(
  page = 1,
): Promise<MyOrdersEnvelope | null> {
  try {
    return await api.get<MyOrdersEnvelope>(
      `/api/v1/me/orders?page=${page}`,
    );
  } catch {
    return null;
  }
}

/** GET /medals 响应（0204 信封化：items + max_worn） */
export interface MedalsEnvelope {
  items: Medal[];
  /** 站点设置的佩戴上限（site_settings.medals_max_worn） */
  max_worn: number;
}

export async function getMedals(): Promise<MedalsEnvelope> {
  try {
    return await api.get<MedalsEnvelope>("/api/v1/medals");
  } catch {
    return { items: [], max_worn: 3 };
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

export async function getMenuItems(
  location: string,
): Promise<MenuItem[]> {
  try {
    // 注意：这里**不能**加跨请求缓存——menu-items 按 optional_auth 的
    // 用户等级过滤（min_class），而 Next data cache 的键不含
    // Authorization 头，缓存会把高等级用户可见的菜单漏给匿名/低等级
    // 用户。保持默认 no-store。
    return await api.get<MenuItem[]>(
      `/api/v1/menu-items?location=${encodeURIComponent(location)}`,
    );
  } catch {
    return [];
  }
}

export type { TorrentListItem, UserPublic };
