/**
 * 种子列表页工具（从 app/(main)/torrents/page.tsx 按域拆出）：
 * loadPublic 匿名公开抓取、toggleSort 表头排序切换、withParam 增量改参。
 * 类型 SectionDictRow / SectionKindMeta / TorrentsSP 一并承载。
 */

import { api } from "@/lib/api-client";

export interface SectionDictRow {
  id: number;
  kind: string;
  name: string;
  sort: number;
}
export interface SectionKindMeta {
  kind: string;
  label: string;
  sort: number;
}

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
export function toggleSort(
  cur: string | undefined,
  key: string,
): string | undefined {
  if (cur === key) return `${key}_asc`;
  if (cur === `${key}_asc`) return undefined;
  return key;
}

// ===== 列表形态（方案阶段二：三视图 + 每页条数可选） =====
// 只影响呈现与单页数量，都不算「筛选条件」，故不进 chips。

export type TorrentView = "table" | "card" | "poster";

export const TORRENT_VIEWS: readonly TorrentView[] = [
  "table",
  "card",
  "poster",
];

/** 归一化视图参数（未知值/缺省 → table，与旧行为一致） */
export function parseView(v: string | undefined): TorrentView {
  return (TORRENT_VIEWS as readonly string[]).includes(v ?? "")
    ? (v as TorrentView)
    : "table";
}

/** 每页条数白名单（与服务端 MAX_LIMIT=100 对齐） */
export const PAGE_SIZES = [20, 50, 100] as const;

export function parsePageSize(v: string | undefined): number {
  const n = Number(v);
  return (PAGE_SIZES as readonly number[]).includes(n) ? n : 20;
}

/**
 * 从「逗号串」多选参数里移除单个值（保留其余已选值）；移除后为空则删掉整个参数。
 * 修复：多选筛选此前每个维度只生成一个 chip，移除即整组清空（选 3 个分类点一下全没了）。
 */
export function withoutValue(
  sp: Record<string, string | undefined>,
  key: string,
  value: string,
): string {
  const kept = (sp[key] ?? "")
    .split(",")
    .map((s) => s.trim())
    .filter((s) => s !== "" && s !== value);
  return withParam(sp, key, kept.length ? kept.join(",") : undefined);
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
