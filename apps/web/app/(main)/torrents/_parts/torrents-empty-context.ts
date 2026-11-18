/**
 * 空态 context 推断（A4）：page.tsx 只负责把 URL 语义交给 EmptyTorrents，
 * 不把判断分支堆进已经很长的路由文件。
 */

import type { TorrentsSP } from "./torrents-utils";

export type TorrentsEmptyContext = "allEmpty" | "filter" | "search" | "dead";

/** 只保留展示态：清空筛选不重置视图/条数（与搜索盒 resetHref 同口径） */
const VIEW_KEYS = ["group", "view", "limit"] as const;

/** 真筛选参数白名单（alive / search / q 单独判；展示态与游标不算筛选） */
const FILTER_KEYS = [
  "category_id",
  "official",
  "status",
  "approval",
  "mine",
  "bookmarked",
  "tag_ids",
  "tag_id",
  "tag_mode",
  "promo",
  "size_min",
  "size_max",
  "min_seeders",
  "max_seeders",
  "min_leechers",
  "max_leechers",
  "min_completed",
  "max_completed",
  "date_from",
  "date_to",
  "rating_min",
  "owner",
  "exclude",
  "anonymous",
  "grade_id",
  "search_area",
  "search_mode",
] as const;

/** 有 q/search → search；有筛选参数 → filter；
 *  仅活种视图（默认或 alive=1）为空 → dead（全是断种）；否则 allEmpty。 */
export function inferEmptyContext(sp: TorrentsSP): TorrentsEmptyContext {
  if (sp.q || sp.search) return "search";
  const filtered = FILTER_KEYS.some((k) => {
    const v = sp[k];
    return Boolean(v) && v !== "0";
  }) || Object.keys(sp).some((k) => k.startsWith("sec_") && Boolean(sp[k]));
  if (filtered) return "filter";
  if (sp.alive !== "0" && sp.alive !== "2") return "dead";
  return "allEmpty";
}

/** 清掉筛选参数回 /torrents：保留 group/view/limit，丢掉游标与分页。 */
export function emptyClearHref(sp: TorrentsSP): string {
  const qs = new URLSearchParams();
  for (const k of VIEW_KEYS) {
    const v = sp[k];
    if (v) qs.set(k, v);
  }
  const s = qs.toString();
  return s ? `/torrents?${s}` : "/torrents";
}
