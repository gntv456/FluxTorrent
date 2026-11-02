/**
 * 用户公开主页·种子列表取数与翻页（从 app/(main)/users/[id]/page.tsx 拆出）：
 * fetchTorrentLists 拉一页种子列表，TorrentListPager 渲染底部翻页条。
 * 均为 server 组件用，无 hooks。
 */

import Link from "next/link";
import { api } from "@/lib/api-client";
import type { Tab, TorrentHistRow, TorrentLists } from "./profile-types";

/** 窄屏一页 20 条（此前一次 limit=100，窄屏那页要滚很久） */
export const TL_PAGE = 20;

/** 六个种子列表 tab（其余 tab 不打 torrentlist 口） */
export type ListTab =
  | "uploads"
  | "seeding"
  | "leeching"
  | "completed"
  | "incomplete"
  | "preserved";

export interface ListView {
  key: ListTab;
  /** 空态文案在 userProfile2 词表里的键 */
  empty: string;
  /** 「我的发布」这一路才带分类与发布时间 */
  withMeta?: boolean;
  publicOnly?: boolean;
}

/** 各 tab 的表头差异（uploads 带分类/时间，其余同列） */
export const LIST_VIEWS: ListView[] = [
  { key: "uploads", empty: "noUploads", withMeta: true, publicOnly: true },
  { key: "seeding", empty: "noUploads" },
  { key: "leeching", empty: "noLeeching" },
  { key: "completed", empty: "noCompleted" },
  { key: "incomplete", empty: "noIncomplete" },
  { key: "preserved", empty: "noPreserved" },
];

export const LIST_TABS: Tab[] = LIST_VIEWS.map((v) => v.key);

/** 拉一页种子列表；多取一条用来判「还有没有下一批」 */
export async function fetchTorrentLists(
  uid: number,
  page: number,
): Promise<{
  torrents: TorrentLists | null;
  /** 每路各自的「还有没有下一批」（翻页条按当前 tab 取，不能共用一个标志） */
  hasMore: Record<string, boolean>;
}> {
  const raw = await api
    .get<TorrentLists>(
      `/api/v1/users/${encodeURIComponent(uid)}` +
        `/torrentlist?limit=${TL_PAGE + 1}` +
        `&offset=${(page - 1) * TL_PAGE}`,
    )
    .catch(() => null);
  if (!raw) return { torrents: null, hasMore: {} };
  const hasMore: Record<string, boolean> = {};
  const cut = (key: string, a: TorrentHistRow[]): TorrentHistRow[] => {
    if (a.length > TL_PAGE) hasMore[key] = true;
    return a.slice(0, TL_PAGE);
  };
  return {
    torrents: {
      uploads: cut("uploads", raw.uploads ?? []),
      seeding: cut("seeding", raw.seeding ?? []),
      leeching: cut("leeching", raw.leeching ?? []),
      completed: cut("completed", raw.completed ?? []),
      incomplete: cut("incomplete", raw.incomplete ?? []),
      preserved: cut("preserved", raw.preserved ?? []),
    },
    hasMore,
  };
}

/** 种子列表翻页条（窄屏那页一次 20 条，底部翻页） */
export function TorrentListPager({
  uid,
  tab,
  page,
  hasMore,
  prevText,
  nextText,
  pageText,
}: {
  uid: number;
  tab: string;
  page: number;
  hasMore: boolean;
  prevText: string;
  nextText: string;
  pageText: string;
}) {
  const href = (p: number) => `/users/${uid}?tab=${tab}&tl_page=${p}`;
  return (
    <nav className="up-tl-pager" aria-label={pageText}>
      {page > 1 ? (
        <Link href={href(page - 1)}>{prevText}</Link>
      ) : (
        <span className="is-dis">{prevText}</span>
      )}
      <span>{pageText.replace("{n}", String(page))}</span>
      {hasMore ? (
        <Link href={href(page + 1)}>{nextText}</Link>
      ) : (
        <span className="is-dis">{nextText}</span>
      )}
    </nav>
  );
}
