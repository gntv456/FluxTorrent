import Link from "next/link";
import { notFound } from "next/navigation";
import { getTopics } from "@/lib/data";
import { TopicComposer } from "@/components/forum-composer";
import { getDict } from "@/i18n/server";
import { dateLocale, fmt } from "@/i18n/config";

export const dynamic = "force-dynamic";

export default async function ForumPage({
  params,
}: {
  params: Promise<{ id: string }>;
}) {
  const { id } = await params;
  const { dict, locale } = await getDict();
  const forumId = Number(id);
  if (!Number.isFinite(forumId)) notFound();
  const data = await getTopics(forumId);
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
          {fmt(dict.forums.boardTitle, { id: forumId })}
        </h1>
        {data.can_create && <TopicComposer forumId={forumId} />}
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
