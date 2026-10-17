import Link from "next/link";
import { EmptyState } from "@/components/ui/empty-state";
import { TypeBadge } from "@/components/forum-bits";
import { Icon } from "@/components/icons";
import { dateLocale, type Locale } from "@/i18n/config";
import type { Dict } from "@/i18n/server";
import type { FeedItem } from "@fluxtorrent/domain-types";

/** 关注流的「关注动态」面板：我关注的版块/作者的新主题 +
 *  我关注主题的最新回复（游标翻页，0123）。 */
export function FeedLatestPanel({
  feed,
  before,
  dict,
  locale,
}: {
  feed: { items: FeedItem[]; next_before: string | null } | null;
  before?: string;
  dict: Dict;
  locale: Locale;
}) {
  const t = dict.forums;
  const t3 = dict.forums3;
  if (!feed) {
    return (
      <EmptyState
        icon="info"
        tone="warn"
        title={t3.failTitle}
        desc={t3.failDesc}
      />
    );
  }
  const items = feed.items;
  const viaLabel = (v: string) =>
    v === "forum" ? t.viaForum : v === "user" ? t.viaUser : t.viaTopic;
  return (
    <div className="flex flex-col gap-3">
      {items.length === 0 ? (
        <p className="rounded-[var(--r-md)] bg-sky-soft p-6 text-center
          text-sm text-sub">
          {t.feedEmpty}
        </p>
      ) : (
        <table className="nexus-table">
          <thead>
            <tr>
              <td className="colhead">{t.topicTitleFallback}</td>
              <td className="colhead hidden w-28 sm:table-cell">{t.via}</td>
              <td className="colhead w-28">{t.author}</td>
              <td className="colhead w-28 text-right">{t.lastPost}</td>
            </tr>
          </thead>
          <tbody>
            {items.map((it) => (
              <tr key={it.topic_id}>
                <td>
                  {it.sticky && (
                    <Icon
                      name="pin"
                      size={13}
                      className="mr-1 inline align-[-2px] text-coral"
                    />
                  )}
                  {it.locked && (
                    <Icon
                      name="lock"
                      size={13}
                      className="mr-1 inline align-[-2px]"
                    />
                  )}
                  <TypeBadge
                    type={it.topic_type}
                    label={
                      it.topic_type
                        ? (dict.forums.types as Record<string, string>)[
                            it.topic_type
                          ]
                        : undefined
                    }
                    className="mr-1 align-middle"
                  />
                  <Link
                    href={`/forums/topic/${it.topic_id}`}
                    className="font-bold text-ink hover:text-sky"
                  >
                    {it.title}
                  </Link>
                  {it.forum_name && (
                    <Link
                      href={`/forums/${it.forum_id}`}
                      className="ml-2 text-xs text-sub hover:text-sky"
                    >
                      » {it.forum_name}
                    </Link>
                  )}
                </td>
                <td className="hidden text-xs text-sub sm:table-cell">
                  {viaLabel(it.via)}
                </td>
                <td className="text-sub">{it.username ?? "—"}</td>
                <td className="num text-right text-sub">
                  {it.last_post_at
                    ? new Date(it.last_post_at).toLocaleDateString(
                        dateLocale(locale),
                      )
                    : "—"}
                </td>
              </tr>
            ))}
          </tbody>
        </table>
      )}
      {/* 游标翻页（0123）：有下一页才渲染；返回链接在翻页后出现 */}
      {feed.next_before && (
        <div className="flex items-center justify-between">
          {before ? (
            <Link
              href="/forums/feed"
              className="text-sm text-sub hover:text-sky"
            >
              ← {t.feedFirst}
            </Link>
          ) : (
            <span />
          )}
          <Link
            href={`/forums/feed?before=${encodeURIComponent(
              feed.next_before,
            )}`}
            className="rounded-[var(--r-md)] border border-line
              bg-[var(--surface-card)] px-4 py-2 text-sm text-sky
              hover:border-sky"
          >
            {t.feedMore} →
          </Link>
        </div>
      )}
    </div>
  );
}
