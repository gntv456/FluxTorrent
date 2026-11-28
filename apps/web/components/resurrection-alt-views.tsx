"use client";

/**
 * 复活任务·非表格视图（2026-11-28 从 resurrection-panel.tsx 拆出守行数门）：
 * card（行卡片）与 poster（海报墙）两视图。数据 DeadRow 与资源库行同构，
 * 视觉复用资源库的 TorrentCards/TorrentPosters（items as never 兼容
 * id/torrent_id 双来源）。
 */
import { fmtCur } from "@/i18n/config";
import Link from "next/link";
import { useI18n } from "@/i18n/client";
import { formatBytes } from "@/lib/format";
import {
  TorrentCards,
  TorrentPosters,
} from "@/app/(main)/torrents/_parts/torrents-cards";
import type { Dict } from "@/i18n/zh-CN";

export function ResurrectionAltViews({
  view,
  items,
  claimed,
  busy,
  onClaim,
  dict,
  colors,
}: {
  view: "table" | "card" | "poster";
  items: unknown[] | null;
  claimed: Record<number, string>;
  busy: number | null;
  onClaim: (torrentId: number) => void;
  dict: Dict;
  colors: Record<number, string>;
}) {
  const { currency } = useI18n();
  const list = (items ?? []) as {
    torrent_id: number;
    name: string;
    small_descr: string | null;
    size: number;
    seeders: number;
    leechers: number;
    times_completed: number;
    category_id: number;
    comments: number;
    created_at: string;
    required_hours: number;
    reward_sparks: number;
  }[];
  if (view === "card") {
    return (
      <TorrentCards items={list as never} dict={dict} colors={colors} />
    );
  }
  if (view === "poster") {
    return (
      <TorrentPosters items={list as never} dict={dict} colors={colors} />
    );
  }
  // table 视图：标准表头（类型/标题/评论/存活/大小/种子/下载数/完成）+ 认领列
  return (
    <div className="brand-wide-table-scroll">
      <table className="nexus-table torrents-table">
        <thead>
          <tr>
            <th className="w-12">{dict.torrents.colType}</th>
            <th>{dict.torrents.colTitle}</th>
            <th className="w-16" title={dict.torrents.colComments}>
              {dict.torrents.colComments}
            </th>
            <th className="w-20" title={dict.torrents.alive}>
              {dict.torrents.alive}
            </th>
            <th className="w-20">{dict.torrents.colSize}</th>
            <th className="w-16">{dict.torrents.colSeeders}</th>
            <th className="w-16">{dict.torrents.colLeechers}</th>
            <th className="w-16">{dict.torrents.colCompleted}</th>
            <th className="w-24">{dict.torrents.colActions}</th>
          </tr>
        </thead>
        <tbody>
          {list.map((r) => {
            const ageDays = Math.floor(
              (Date.now() - new Date(r.created_at).getTime()) / 86400000,
            );
            return (
              <tr key={r.torrent_id}>
                <td className="w-12 text-sub">#{r.category_id}</td>
                <td>
                  <Link href={`/torrent/${r.torrent_id}`} className="text-link">
                    {r.name}
                  </Link>
                  <div className="text-xs text-sub">
                    {fmtCur(dict.resurrect.contract, {
                      h: String(r.required_hours),
                      sparks: String(r.reward_sparks),
                    }, currency)}
                  </div>
                  {r.small_descr && (
                    <div className="text-xs text-sub">{r.small_descr}</div>
                  )}
                </td>
                <td className="num text-sub">{r.comments || ""}</td>
                <td className="num text-sub">
                  {ageDays > 0 ? `${ageDays}d` : ""}
                </td>
                <td className="num text-sub">{formatBytes(r.size)}</td>
                <td className="num">{r.seeders}</td>
                <td className="num">{r.leechers}</td>
                <td className="num">{r.times_completed}</td>
                <td className="text-right">
                  {claimed[r.torrent_id] ? (
                    <span className="text-xs font-bold text-mint">
                      {claimed[r.torrent_id]}
                    </span>
                  ) : (
                    <button
                      type="button"
                      disabled={busy !== null}
                      onClick={() => onClaim(r.torrent_id)}
                      className="min-h-11 rounded-full border border-[var(--compat-orange-dark)] px-3 text-xs font-bold text-[var(--compat-orange-dark)] disabled:opacity-50"
                    >
                      {dict.resurrect.claimBtn}
                    </button>
                  )}
                </td>
              </tr>
            );
          })}
          {list.length === 0 && (
            <tr>
              <td colSpan={9} className="py-6 text-center text-sub">
                {dict.resurrect.empty}
              </td>
            </tr>
          )}
        </tbody>
      </table>
    </div>
  );
}
