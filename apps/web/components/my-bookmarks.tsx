"use client";

import Link from "next/link";
import { useI18n } from "@/i18n/client";
import { dateLocale, fmt } from "@/i18n/config";
import { formatBytes } from "@/lib/format";
import { ReadState } from "@/components/ui/read-state";
import { BookmarkRemoveButton } from "@/components/bookmark-remove-button";
import type { BookmarkRow } from "@/lib/data";

/** 追更中心第三档「我的收藏」（样图 § subscriptions）。
 *  样图画的是「收藏日历」月历格，那需要一个按月聚合的新端点；这里先按
 *  常规 PT 口径做清单（同一份 GET /me/bookmarks 数据，行内可取消收藏），
 *  日历形态留待有聚合口再叠。
 *
 *  后端 limit 截断、无 cursor（取满 100 就要说出口，同 /my/torrentlist）。 */
const CAP = 100;

/** 固定站点时区（UTC+8，与站点日/签到同口径）：RSC 在容器（UTC）里也会
 *  渲染这一列，只有两端都用同一固定时区才不会水合出不同文本。 */
function fmtSavedAt(iso: string, locale: string): string {
  return new Date(iso).toLocaleString(locale, {
    timeZone: "Asia/Shanghai",
  });
}

export function MyBookmarks({ rows }: { rows: BookmarkRow[] | null }) {
  const { dict, locale } = useI18n();
  const t = dict.subscriptions;
  const list = rows ?? [];
  return (
    <ReadState
      failed={rows === null}
      empty={list.length === 0}
      dict={dict}
      emptyTitle={t.bookmarksEmpty}
      emptyDesc={t.bookmarksEmptyHint}
      emptyIcon="star"
    >
      <div className="baozi-panel overflow-x-auto">
        <table className="nexus-table">
          <thead>
            <tr>
              <td className="colhead">{t.colName}</td>
              <td className="colhead w-24 text-right">
                {dict.mytl.colSize}
              </td>
              <td className="colhead w-16 text-right">
                {dict.mytl.colSeeders}
              </td>
              <td className="colhead w-16 text-right">
                {dict.mytl.colLeechers}
              </td>
              <td className="colhead w-40">{t.colSavedAt}</td>
              <td className="colhead w-24 text-right">{t.colAction}</td>
            </tr>
          </thead>
          <tbody>
            {list.map((r) => (
              <tr key={r.torrent_id} data-torrent-id={r.torrent_id}>
                <td className="max-w-[360px]">
                  <Link
                    href={`/torrent/${r.torrent_id}`}
                    className={
                      "block truncate font-bold text-ink " +
                      "hover:text-[var(--sky-deep)]"
                    }
                    title={r.name}
                  >
                    {r.name}
                  </Link>
                  {r.small_descr && (
                    <span className="block truncate text-xs text-sub">
                      {r.small_descr}
                    </span>
                  )}
                </td>
                <td className="num text-right text-xs">
                  {formatBytes(r.size)}
                </td>
                <td className="num text-right">{r.seeders}</td>
                <td className="num text-right">{r.leechers}</td>
                <td className="text-xs text-sub">
                  {fmtSavedAt(r.created_at, dateLocale(locale))}
                </td>
                <td className="text-right">
                  <BookmarkRemoveButton torrentId={r.torrent_id} />
                </td>
              </tr>
            ))}
          </tbody>
        </table>
        {list.length >= CAP && (
          <p className="px-3 py-2 text-xs text-sub">
            {fmt(dict.mytl.truncated, { n: CAP })}
          </p>
        )}
      </div>
    </ReadState>
  );
}
