/**
 * 列表形态切换（方案阶段二）：视图（列表/卡片/海报墙）+ 每页条数（20/50/100）。
 *
 * 两种偏好都写进 URL（可分享、可回退、刷新不丢），与筛选参数同一套 withParam。
 * 切换时**清掉 cursor**：条数/视图变了还停在上一次的游标上会让人以为「翻页丢内容」。
 * 无 hooks，保持 server component（纯链接，走整页 SSR 与筛选一致）。
 */

import type { Dict } from "@/i18n/zh-CN";
import {
  PAGE_SIZES,
  TORRENT_VIEWS,
  type TorrentView,
  type TorrentsSP,
} from "./torrents-utils";

export function TorrentsViewSwitch({
  dict,
  sp,
  view,
  pageSize,
  withParam,
}: {
  dict: Dict;
  sp: TorrentsSP;
  view: TorrentView;
  pageSize: number;
  withParam: (sp: TorrentsSP, key: string, value: string | undefined) => string;
}) {
  const t = dict.torrents;
  const labels: Record<TorrentView, string> = {
    table: t.viewTable,
    card: t.viewCard,
    poster: t.viewPoster,
  };
  // 换形态 = 回到第一页：先把 cursor 从参数集里剔除再拼链接
  // （游标是「(排序键,id)」位置，形态/条数变了继续用它只会让人以为翻页丢内容）
  const base = Object.fromEntries(
    Object.entries(sp).filter(([k]) => k !== "cursor"),
  ) as TorrentsSP;
  const href = (key: string, value: string | undefined) =>
    withParam(base, key, value);

  return (
    <div className="flex flex-wrap items-center justify-between gap-2">
      <div
        className={
          "flex items-center gap-0.5 rounded-full border border-line " +
          "bg-[var(--surface-card)] p-0.5"
        }
        role="group"
        aria-label={t.viewLabel}
      >
        {TORRENT_VIEWS.map((v) => {
          const on = v === view;
          return (
            <a
              key={v}
              href={href("view", v === "table" ? undefined : v)}
              aria-current={on ? "page" : undefined}
              className={`rounded-full px-3 py-1 text-xs transition-colors ${
                on
                  ? "bg-[var(--sky-soft)] font-semibold text-sky-deep"
                  : "text-sub hover:text-ink"
              }`}
            >
              {labels[v]}
            </a>
          );
        })}
      </div>
      <div className="flex items-center gap-1 text-xs text-sub">
        {PAGE_SIZES.map((n) => {
          const on = n === pageSize;
          return (
            <a
              key={n}
              href={href("limit", n === 20 ? undefined : String(n))}
              aria-current={on ? "page" : undefined}
              className={`num rounded-full px-2 py-1 transition-colors ${
                on
                  ? "bg-[var(--sky-soft)] font-semibold text-sky-deep"
                  : "hover:text-ink"
              }`}
              title={t.perPage.replace("{n}", String(n))}
            >
              {n}
            </a>
          );
        })}
      </div>
    </div>
  );
}
