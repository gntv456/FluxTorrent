"use client";

/** 首页排版相关常量与解析（从 home-sections.tsx 按域拆出，300 门禁）：
 *  home_layout 的键白名单 / 默认布局 / 解析与宽度档计算。
 *  （home-layout-editor.tsx 也从这里取用） */

export interface HomeLayoutItem {
  key: string;
  span: number;
}

export const HOME_SECTION_KEYS = [
  "news",
  "attendance",
  "shoutbox",
  "funbox",
  "resource_stats",
  "site_data",
  "lucky_draw",
  "links",
  "latest",
] as const;

export const DEFAULT_HOME_LAYOUT: HomeLayoutItem[] = [
  { key: "news", span: 0 },
  { key: "attendance", span: 0 },
  { key: "shoutbox", span: 0 },
  { key: "funbox", span: 0 },
  { key: "resource_stats", span: 3 },
  { key: "site_data", span: 0 },
  { key: "lucky_draw", span: 0 },
  { key: "links", span: 3 },
];

/** 推荐宽度档：未显式指定 span 的板块按此渲染（保持默认视觉） */
const RECOMMENDED_SPAN: Record<string, number> = {
  news: 2,
  attendance: 1,
  shoutbox: 2,
  funbox: 1,
  resource_stats: 3,
  site_data: 2,
  lucky_draw: 1,
  links: 3,
  latest: 3,
};

/** 解析后端 home_layout：结构/键非法整体回退默认（后台保存端点已强校验，
 *  这里兜底手改库或历史脏数据） */
export function parseHomeLayout(raw: string | undefined | null): HomeLayoutItem[] {
  if (!raw || !raw.trim()) return DEFAULT_HOME_LAYOUT;
  try {
    const arr = JSON.parse(raw) as { key?: string; span?: number }[];
    if (!Array.isArray(arr) || arr.length === 0) return DEFAULT_HOME_LAYOUT;
    const items: HomeLayoutItem[] = [];
    const seen = new Set<string>();
    for (const it of arr) {
      if (!it?.key || typeof it.key !== "string") return DEFAULT_HOME_LAYOUT;
      if (!(HOME_SECTION_KEYS as readonly string[]).includes(it.key)) return DEFAULT_HOME_LAYOUT;
      if (seen.has(it.key)) return DEFAULT_HOME_LAYOUT;
      seen.add(it.key);
      const span = typeof it.span === "number" && [0, 1, 2, 3].includes(it.span) ? it.span : 0;
      items.push({ key: it.key, span });
    }
    return items;
  } catch {
    return DEFAULT_HOME_LAYOUT;
  }
}

/** 板块实际宽度档：显式 span 优先，0 = 推荐档 */
export function effectiveSpan(item: HomeLayoutItem): number {
  return item.span || RECOMMENDED_SPAN[item.key] || 3;
}
