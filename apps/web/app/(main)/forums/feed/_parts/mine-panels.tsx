import Link from "next/link";
import { EmptyState } from "@/components/ui/empty-state";
import { Pager } from "@/components/ui/pager";
import { dateLocale, fmt, type Locale } from "@/i18n/config";
import type { Dict } from "@/i18n/server";
import type {
  ForumMine,
  MyReplyRow,
  MyTopicRow,
} from "@fluxtorrent/domain-types";

/** 关注流的「我参与的 / 我的回复」两个标签面板（RSC，翻页走链接）。
 *
 *  三档分开说：接口没取到 → 出声；取到但为空 → 才说「还没有」；
 *  有数据 → 表格。把失败演成空态是这个域里最坏的一类假状态
 *  （用户会以为自己没发过帖，而不是以为站点坏了）。 */

const day = (iso: string | null, locale: Locale) =>
  iso ? new Date(iso).toLocaleDateString(dateLocale(locale)) : "—";

/** 越界页回空数组时，total 仍如实报——页码照画，用户能翻回去。 */
function pagerOf(
  data: ForumMine,
  base: string,
): { page: number; pages: number; total: number; hrefs: string[] } {
  const pages = Math.max(1, Math.ceil(data.total / data.per));
  const hrefs = Array.from({ length: pages }, (_, i) =>
    i === 0 ? base : `${base}?page=${i + 1}`,
  );
  return { page: data.page, pages, total: data.total, hrefs };
}

function FailNote({ t }: { t: Dict["forums3"] }) {
  return (
    <EmptyState
      icon="info"
      tone="warn"
      title={t.failTitle}
      desc={t.failDesc}
    />
  );
}

export function MyTopicsPanel({
  data,
  dict,
  locale,
}: {
  data: ForumMine | null;
  dict: Dict;
  locale: Locale;
}) {
  const t = dict.forums3;
  const f = dict.forums;
  if (!data) return <FailNote t={t} />;
  const rows = data.items as MyTopicRow[];
  if (rows.length === 0) {
    return (
      <EmptyState
        compact
        icon="folder"
        title={t.mineEmptyTopics}
        desc={t.mineEmptyTopicsHint}
      />
    );
  }
  const p = pagerOf(data, "/forums/feed?tab=mine");
  return (
    <div className="flex flex-col gap-3">
      <table className="nexus-table">
        <thead>
          <tr>
            <td className="colhead">{f.topicTitleFallback}</td>
            <td className="colhead hidden w-28 sm:table-cell">{t.colBoard}</td>
            <td className="colhead w-28 text-right">{t.colReplies}</td>
            <td className="colhead w-28 text-right">{f.lastPost}</td>
          </tr>
        </thead>
        <tbody>
          {rows.map((r) => (
            <tr key={r.id}>
              <td>
                <Link
                  href={`/forums/topic/${r.id}`}
                  className="font-bold text-ink hover:text-sky"
                >
                  {r.title}
                </Link>
                {r.my_replies > 0 && (
                  <span className="ml-2 text-xs text-sub">
                    {fmt(t.myRepliesN, { n: r.my_replies })}
                  </span>
                )}
              </td>
              <td className="hidden text-xs text-sub sm:table-cell">
                <Link
                  href={`/forums/${r.forum_id}`}
                  className="hover:text-sky"
                >
                  {r.forum_name}
                </Link>
              </td>
              <td className="num text-right text-sub">{r.replies}</td>
              <td className="num text-right text-sub">
                {day(r.last_post_at ?? r.created_at, locale)}
              </td>
            </tr>
          ))}
        </tbody>
      </table>
      {p.pages > 1 && (
        <Pager
          page={p.page}
          pages={p.pages}
          total={p.total}
          hrefs={p.hrefs}
        />
      )}
    </div>
  );
}

export function MyRepliesPanel({
  data,
  dict,
  locale,
}: {
  data: ForumMine | null;
  dict: Dict;
  locale: Locale;
}) {
  const t = dict.forums3;
  const f = dict.forums;
  if (!data) return <FailNote t={t} />;
  const rows = data.items as MyReplyRow[];
  if (rows.length === 0) {
    return (
      <EmptyState
        compact
        icon="folder"
        title={t.mineEmptyReplies}
        desc={t.mineEmptyTopicsHint}
      />
    );
  }
  const p = pagerOf(data, "/forums/feed?tab=replies");
  return (
    <div className="flex flex-col gap-3">
      <table className="nexus-table">
        <thead>
          <tr>
            <td className="colhead">{f.topicTitleFallback}</td>
            <td className="colhead">{t.colExcerpt}</td>
            <td className="colhead w-28 text-right">{f.lastPost}</td>
          </tr>
        </thead>
        <tbody>
          {rows.map((r) => (
            <tr key={r.id}>
              <td>
                <Link
                  href={`/forums/topic/${r.topic_id}`}
                  className="font-bold text-ink hover:text-sky"
                >
                  {r.topic_title}
                </Link>
                <Link
                  href={`/forums/${r.forum_id}`}
                  className="mt-0.5 block text-xs text-sub hover:text-sky"
                >
                  » {r.forum_name}
                </Link>
              </td>
              <td className="text-xs text-body">{r.excerpt}…</td>
              <td className="num text-right text-sub">
                {day(r.created_at, locale)}
              </td>
            </tr>
          ))}
        </tbody>
      </table>
      {p.pages > 1 && (
        <Pager page={p.page} pages={p.pages} total={p.total} hrefs={p.hrefs} />
      )}
    </div>
  );
}
