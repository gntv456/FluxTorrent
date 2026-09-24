/**
 * 用户公开主页·标签页内容（从 app/(main)/users/[id]/page.tsx 按域拆出）：
 * renderTorrentList 种子列表 tab 表格、PostsTab 论坛动态、CommentsTab 种子评论。
 * 无 hooks：server component，数据经 props 注入。
 */

import Link from "next/link";
import type { Locale } from "@/i18n/config";
import { dateLocale } from "@/i18n/config";
import type { ProfileData, TorrentHistRow } from "./profile-types";

/** 种子列表 tab 渲染（每 tab 复用同一张表：名称/大小 + S/L；uploads 标注仅公开） */
export function TorrentListTable({
  rows,
  emptyText,
  t,
  gb,
  publicOnly = false,
}: {
  rows: TorrentHistRow[];
  emptyText: string;
  t: Record<string, string>;
  gb: (n: number) => string;
  publicOnly?: boolean;
}) {
  return (
    <div className="baozi-wide-table-scroll">
      <table className="nexus-table">
        <thead>
          <tr>
            <td className="colhead">
              {`${t.torrentName}${publicOnly ? `（${t.histPublicOnly}）` : ""}`}
            </td>
            <td className="colhead w-28 text-right">{t.colSize}</td>
            <td className="colhead w-20 text-right">{t.colSL}</td>
          </tr>
        </thead>
        <tbody>
          {rows.map((u) => (
            <tr key={u.torrent_id}>
              <td className="max-w-[420px] truncate">
                <Link
                  href={`/torrent/${u.torrent_id}`}
                  className="text-sky-deep hover:underline"
                >
                  {u.name}
                </Link>
              </td>
              <td className="num shrink-0 text-right text-sub">{gb(u.size)}</td>
              <td className="num shrink-0 text-right text-sub">
                {u.seeders} / {u.leechers}
              </td>
            </tr>
          ))}
          {rows.length === 0 && (
            <tr>
              <td colSpan={3} className="py-6 text-center text-sub">
                {emptyText}
              </td>
            </tr>
          )}
        </tbody>
      </table>
    </div>
  );
}

export function PostsTab({
  data,
  locale,
  t,
}: {
  data: ProfileData;
  locale: Locale;
  t: Record<string, string>;
}) {
  return (
    <section className="nexus-detail">
      <table className="nexus-table">
        <thead>
          <tr>
            <td className="colhead">{t.recentPosts}</td>
          </tr>
        </thead>
        <tbody>
          {data.recent_posts.map(([pid, topicId, body, at]) => (
            <tr key={pid}>
              <td>
                <Link
                  href={`/forums/topic/${topicId}`}
                  className="text-xs font-bold text-sky"
                >
                  #{topicId}
                </Link>
                <p className="text-sm">{body}</p>
                <p className="text-[11px] text-sub">
                  {new Date(at).toLocaleString(dateLocale(locale))}
                </p>
              </td>
            </tr>
          ))}
          {data.recent_posts.length === 0 && (
            <tr>
              <td className="py-6 text-center text-sub">{t.noPosts}</td>
            </tr>
          )}
        </tbody>
      </table>
    </section>
  );
}

export function CommentsTab({
  data,
  locale,
  t,
}: {
  data: ProfileData;
  locale: Locale;
  t: Record<string, string>;
}) {
  return (
    <section className="nexus-detail">
      <table className="nexus-table">
        <thead>
          <tr>
            <td className="colhead">{t.recentComments}</td>
          </tr>
        </thead>
        <tbody>
          {data.recent_comments.map((c, i) => (
            <tr key={i}>
              <td>
                <Link
                  href={`/torrent/${c.torrent_id}`}
                  className="text-xs font-bold text-sky"
                >
                  #{c.torrent_id}
                </Link>
                <p className="text-sm whitespace-pre-wrap">{c.body}</p>
                <p className="text-[11px] text-sub">
                  {new Date(c.created_at).toLocaleString(dateLocale(locale))}
                </p>
              </td>
            </tr>
          ))}
          {data.recent_comments.length === 0 && (
            <tr>
              <td className="py-6 text-center text-sub">{t.noComments}</td>
            </tr>
          )}
        </tbody>
      </table>
    </section>
  );
}
