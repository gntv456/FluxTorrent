import Link from "next/link";
import { notFound } from "next/navigation";
import { getTopics } from "@/lib/data";
import { TopicComposer } from "@/components/forum-composer";
import { TypeBadge } from "@/components/forum-bits";
import { FollowButton } from "@/components/forum-follow";
import { getDict } from "@/i18n/server";
import { dateLocale, fmt } from "@/i18n/config";

export const dynamic = "force-dynamic";

export default async function ForumPage({
  params,
  searchParams,
}: {
  params: Promise<{ id: string }>;
  searchParams: Promise<{ sort?: string }>;
}) {
  const { id } = await params;
  const { sort: sortRaw } = await searchParams;
  const { dict, locale } = await getDict();
  const forumId = Number(id);
  if (!Number.isFinite(forumId)) notFound();
  // 排序白名单（0116）：hot=热度衰减 / 缺省 new=最新
  const sort = sortRaw === "hot" ? "hot" : "new";
  const data = await getTopics(forumId, sort);
  // 版块不存在或无 minclassread 门槛权限（Forbidden）→ 404 口径
  if (!data) notFound();
  const topics = data.topics;

  return (
    <div className="flex flex-col gap-4">
      <div className="flex items-baseline gap-2">
        <Link href="/forums" className="text-sm text-sky">
          {dict.forums.backToForums}
        </Link>
      </div>
      <div className="flex flex-wrap items-center justify-between gap-2">
        <h1 className="font-display text-2xl">
          {data.forum_name ?? fmt(dict.forums.boardTitle, { id: forumId })}
        </h1>
        <div className="flex items-center gap-2">
          <FollowButton targetType="forum" targetId={forumId} showCount />
          {/* 排序切换：SSR 链接，不引入客户端状态 */}
          <div className="inline-flex overflow-hidden rounded-full border border-line">
            <Link
              href={`/forums/${forumId}?sort=new`}
              aria-current={sort === "new" ? "true" : undefined}
              className={`px-3 py-1 text-xs font-bold transition ${
                sort === "new"
                  ? "bg-[var(--sky-soft)] text-sky"
                  : "text-sub hover:text-sky"
              }`}
            >
              {dict.forums.sortNew}
            </Link>
            <Link
              href={`/forums/${forumId}?sort=hot`}
              aria-current={sort === "hot" ? "true" : undefined}
              className={`px-3 py-1 text-xs font-bold transition ${
                sort === "hot"
                  ? "bg-[var(--sky-soft)] text-sky"
                  : "text-sub hover:text-sky"
              }`}
            >
              {dict.forums.sortHot}
            </Link>
          </div>
          {data.can_create && <TopicComposer forumId={forumId} />}
        </div>
      </div>
      {topics.length === 0 ? (
        <p className="py-10 text-center text-sub">{dict.forums.noTopics}</p>
      ) : (
        <table className="nexus-table">
          <thead>
            <tr>
              <td className="colhead">{dict.forums.topicTitleFallback}</td>
              <td className="colhead w-32">{dict.forums.author}</td>
              <td className="colhead w-16 text-right">{dict.forums.replies.replace("{n}", "").trim() || dict.forums.replies}</td>
              <td className="colhead hidden w-20 text-right sm:table-cell">
                {dict.forums.views.replace("{n}", "").trim() || dict.forums.views}
              </td>
              <td className="colhead hidden w-28 text-right sm:table-cell">
                {dict.forums.lastPost}
              </td>
            </tr>
          </thead>
          <tbody>
            {topics.map((t) => (
              <tr key={t.id}>
                <td>
                  {t.digest && (
                    <span aria-label="精华" className="mr-1 text-[var(--baozi-orange-dark)]">
                      ⭐
                    </span>
                  )}
                  {t.sticky && (
                    <span className="mr-1 text-xs font-bold text-coral" title="置顶">
                      📌
                    </span>
                  )}
                  {t.locked && (
                    <span className="mr-1 text-xs" title="已锁定">
                      🔒
                    </span>
                  )}
                  <TypeBadge
                    type={t.topic_type}
                    label={
                      t.topic_type
                        ? (dict.forums.types as Record<string, string>)[t.topic_type]
                        : undefined
                    }
                    className="mr-1 align-middle"
                  />
                  <Link
                    href={`/forums/topic/${t.id}`}
                    className="font-bold text-ink hover:text-sky"
                  >
                    {t.title}
                  </Link>
                </td>
                <td className="text-sub">{t.username ?? "—"}</td>
                <td className="num text-right">{Math.max(t.replies, 0)}</td>
                <td className="num hidden text-right sm:table-cell">
                  {t.views}
                </td>
                <td className="num hidden text-right text-sub sm:table-cell">
                  {t.last_post_at
                    ? new Date(t.last_post_at).toLocaleDateString(
                        dateLocale(locale),
                      )
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
