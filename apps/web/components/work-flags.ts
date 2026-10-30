/**
 * 作品层展示信息（徽标/评分/主创）：从作品行 + 组内前 8 条种子推导。
 *
 * 画质/促销/官方只存在于种子名与种子行，作品层没有结构化字段；这里单源
 * 解析，卡片、列表、海报墙共用。只吃接口已下发的前 8 条种子——第 9 条
 * 之后的版本可能带不同画质/促销，不额外请求。
 */

import type { WorkWithSeeds } from "@fluxtorrent/domain-types";
import type { Dict } from "@/i18n/zh-CN";
import { promotionBadge } from "@/lib/format";
import { nameChips } from "@/components/work-chips";

const RES_ORDER = ["4K", "1080P", "720P"];
const PROMO_ORDER = ["free", "x2free", "x2", "half", "x2half", "p30"];

export interface WorkFlagData {
  /** 作品内出现过的画质（最多两档，4K → 1080P → 720P） */
  qualities: string[];
  /** 作品内仍在进行的促销（Free/2x 等，最多两档） */
  promos: { key: string; label: string; tone: string }[];
  /** 是否含官方版本 */
  official: boolean;
  /** 豆瓣（media_info.rating）与 TMDB（work_meta.rating）评分 */
  ratings: { label: string; value: string }[];
  /** work_meta 里可展示的主创/条目信息；缺 meta 时为空 */
  genres: string[];
  region: string | null;
  runtime: number | null;
  director: string[];
  cast: string[];
}

/** 作品层可展示信息（缺 meta/种子时给空值，不拿标题去猜） */
export function workFlags(
  work: WorkWithSeeds,
  dict: Dict,
): WorkFlagData {
  const qualities = new Set<string>();
  for (const name of [work.name, ...work.seeds.map((s) => s.name)]) {
    const hit = nameChips(name, dict.works.chipSubs)
      .find((c) => c.cls === "wk-c4k");
    if (hit) qualities.add(hit.text);
  }

  const promos = new Map<string, { label: string; tone: string }>();
  for (const s of work.seeds) {
    const badge = promotionBadge(s.promotion);
    if (!badge) continue;
    const label = dict.promotion[badge.key];
    if (!label) continue;
    promos.set(badge.key, {
      label,
      tone: badge.tone,
    });
  }

  const meta = work.meta?.source ? work.meta : null;
  const ratings: { label: string; value: string }[] = [];
  const douban = work.rating?.trim();
  if (douban) {
    ratings.push({
      label: work.douban_id ? dict.works.douban : dict.works.rating,
      value: douban,
    });
  }
  if (meta?.rating != null) {
    ratings.push({
      label: dict.works.tmdb,
      value: meta.rating.toFixed(1),
    });
  }

  return {
    qualities: [...qualities]
      .sort((a, b) => RES_ORDER.indexOf(a) - RES_ORDER.indexOf(b))
      .slice(0, 2),
    promos: [...promos]
      .sort(
        ([a], [b]) =>
          PROMO_ORDER.indexOf(a) - PROMO_ORDER.indexOf(b),
      )
      .slice(0, 2)
      .map(([key, p]) => ({ key, ...p })),
    official: work.seeds.some((s) => s.official),
    ratings: ratings.slice(0, 2),
    genres: (meta?.genres ?? []).filter(Boolean),
    region: meta?.region ?? null,
    runtime: meta?.runtime ?? null,
    director: (meta?.director ?? []).filter(Boolean),
    cast: (meta?.cast_list ?? []).filter(Boolean),
  };
}
