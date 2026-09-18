import Link from "next/link";
import { getFeed, getMyFollows } from "@/lib/data";
import { TypeBadge } from "@/components/forum-bits";
import { getDict } from "@/i18n/server";
import { dateLocale } from "@/i18n/config";

export const dynamic = "force-dynamic";

/**
 * 关注流（0121）：我关注的版块/作者的新主题 + 我关注主题的最新回复，
 * 按最后活动时间倒序。`via` 标出这条是靠哪种关注命中的，便于用户理解流里为什么有它。
 */
export default async function FeedPage() {
  const { dict, locale } = await getDict();
  const t = dict.forums;
  const [items, mine] = await Promise.all([getFeed(50), getMyFollows()]);
  const viaLabel = (v: string) =>
    v === "forum" ? t.viaForum : v === "user" ? t.viaUser : t.viaTopic;
  const viaCount = mine.forums.length + mine.users.length + mine.topics.length;

  return (
    <div className="flex flex-col gap-4">
      <div className="flex flex-wrap items-baseline justify-between gap-2">
        <h1 className="font-display text-2xl">{t.feedTitle}</h1>
        <div className="flex items-baseline gap-3 text-sm">
          <Link href="/forums" className="text-sky hover:underline">
            {t.backToForums}
          </Link>
        </div>
      </div>

      {/* 我的关注概况：告诉用户流是从哪来的，也给出取消入口（取消在各自页面上的关注按钮） */}
      {viaCount > 0 && (
        <div className="rounded-[var(--r-md)] border border-line bg-[var(--surface-card)] p-3 text-xs text-sub">
          <span className="font-bold text-ink">{t.myFollows}</span>
          {mine.forums.length > 0 && (
            <span className="ml-2">
              {t.viaForum} ×{mine.forums.length}
            </span>
          )}
          {mine.users.length > 0 && (
            <span className="ml-2">
              {t.viaUser} ×{mine.users.length}
            </span>
          )}
          {mine.topics.length > 0 && (
            <span className="ml-2">
              {t.viaTopic} ×{mine.topics.length}
            </span>
          )}
        </div>
      )}

      {items.length === 0 ? (
        <p className="rounded-[var(--r-md)] bg-sky-soft p-6 text-center text-sm text-sub">
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
                  {it.sticky && <span className="mr-1 text-xs text-coral">📌</span>}
                  {it.locked && <span className="mr-1 text-xs">🔒</span>}
                  <TypeBadge
                    type={it.topic_type}
                    label={
                      it.topic_type
                        ? (dict.forums.types as Record<string, string>)[it.topic_type]
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
                <td className="hidden text-xs text-sub sm:table-cell">{viaLabel(it.via)}</td>
                <td className="text-sub">{it.username ?? "—"}</td>
                <td className="num text-right text-sub">
                  {it.last_post_at
                    ? new Date(it.last_post_at).toLocaleDateString(dateLocale(locale))
                    : "—"}
                </td>
              </tr>
            ))}
          </tbody>
        </table>
      )}
    </div>
  );
}
