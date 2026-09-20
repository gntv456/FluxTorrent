/**
 * 种子列表页工具（从 app/(main)/torrents/page.tsx 按域拆出）：
 * loadPublic 匿名公开抓取、toggleSort 表头排序切换、withParam 增量改参。
 * 类型 SectionDictRow / SectionKindMeta / TorrentsSP 一并承载。
 */

import { api } from "@/lib/api-client";

export interface SectionDictRow { id: number; kind: string; name: string; sort: number }
export interface SectionKindMeta { kind: string; label: string; sort: number }

/** 归一化后的种子页查询参数（重复参数合并为逗号串） */
export type TorrentsSP = Record<string, string | undefined>;

/** 匿名公开接口统一抓取（api.get 自带 SSR 内网直连 API_SERVER_URL；失败时隐藏对应筛选区） */
export async function loadPublic<T>(path: string): Promise<T | null> {
  try {
    return await api.get<T>(path);
  } catch {
    return null;
  }
}

/** 表头排序切换：当前列降序 → 升序（_asc）→ 取消；其他列 → 降序 */
export function toggleSort(cur: string | undefined, key: string): string | undefined {
  if (cur === key) return `${key}_asc`;
  if (cur === `${key}_asc`) return undefined;
  return key;
}

/** 在现有参数上增量修改，保留其余筛选（修复翻页丢参数） */
export function withParam(
  sp: Record<string, string | undefined>,
  key: string,
  value: string | undefined,
): string {
  const qs = new URLSearchParams();
  for (const [k, v] of Object.entries(sp)) {
    if (v !== undefined && v !== "" && k !== key) qs.set(k, v);
  }
  if (value !== undefined && value !== "") qs.set(key, value);
  const s = qs.toString();
  return s ? `/torrents?${s}` : "/torrents";
}
