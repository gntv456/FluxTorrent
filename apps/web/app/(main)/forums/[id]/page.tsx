import Link from "next/link";
import { notFound } from "next/navigation";
import { getTopics, getForumTags } from "@/lib/data";
import { TopicComposer } from "@/components/forum-composer";
import { TypeBadge, TagChip } from "@/components/forum-bits";
import { Icon } from "@/components/icons";
import { FollowButton } from "@/components/forum-follow";
import { getDict } from "@/i18n/server";
import { dateLocale, fmt } from "@/i18n/config";

export const dynamic = "force-dynamic";

export default async function ForumPage({
  params,
  searchParams,
}: {
  params: Promise<{ id: string }>;
  searchParams: Promise<{ sort?: string; tag?: string }>;
}) {
  const { id } = await params;
  const { sort: sortRaw, tag: tagRaw } = await searchParams;
  const { dict, locale } = await getDict();
  const forumId = Number(id);
  if (!Number.isFinite(forumId)) notFound();
  // 排序白名单（0116）：hot=热度衰减 / 缺省 new=最新
  const sort = sortRaw === "hot" ? "hot" : "new";
  // 标签筛选（0123）：tag=tag_dict id；非法值回落全量
  const tagNum = Number(tagRaw);
  const tag = Number.isFinite(tagNum) && tagNum > 0 ? tagNum : undefined;
  const [data, tagDict] = await Promise.all([
    getTopics(forumId, sort, tag),
    getForumTags(),
  ]);
  // 版块不存在或无 minclassread 门槛权限（Forbidden）→ 404 口径
  if (!data) notFound();
  const topics = data.topics;
  const activeTag = tagDict.find((t) => t.id === tag);
  // 筛选/排序链接共享的 query 基（保持 tag 与 sort 互不丢失）
  const qs = (over: Record<string, string | undefined>) => {
    const sp = new URLSearchParams();
    const merged = { sort, tag: tag ? String(tag) : undefined, ...over };
    for (const [k, v] of Object.entries(merged)) if (v) sp.set(k, v);
    const s = sp.toString();
    return s ? `?${s}` : "";
  };

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
          <div className={
            "inline-flex overflow-hidden rounded-full border border-line"
          }>
            <Link
              href={`/forums/${forumId}${qs({ sort: "new" })}`}
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
              href={`/forums/${forumId}${qs({ sort: "hot" })}`}
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
      {/* 标签筛选（0123）：SSR 链接切换，选中项加 outline 高亮；「全部」清除筛选 */}
      {tagDict.length > 0 && (
        <div className="flex flex-wrap items-center gap-1.5">
          <Link
            href={`/forums/${forumId}${qs({ tag: undefined })}`}
            aria-current={!tag ? "true" : undefined}
            className={`rounded-full border px-2.5 py-0.5 text-[11px] font-bold ${

              !tag
                ? "border-sky bg-[var(--sky-soft)] text-sky"
                : "border-line text-sub hover:text-sky"
            }`}
          >
            {dict.forums.tagAll}
          </Link>
          {tagDict.map((t) =>
            tag === t.id ? (
              <Link
                key={t.id}
                href={`/forums/${forumId}${qs({ tag: undefined })}`}
                aria-current="true"
                className={
            "rounded-full outline outline-2 outline-offset-1 outline-sky"
          }
                title={dict.forums.tagClear}
              >
                <TagChip tag={t} />
              </Link>
            ) : (
              <Link
                key={t.id}
                href={`/forums/${forumId}${qs({ tag: String(t.id) })}`}
                className={
            "rounded-full opacity-70 transition hover:opacity-100"
          }
              >
                <TagChip tag={t} />
              </Link>
            ),
          )}
        </div>
      )}
      {activeTag && topics.length === 0 && (
        <p className="py-6 text-center text-sm text-sub">
          {dict.forums.tagNoHit}
        </p>
      )}
      {topics.length === 0 ? (
        <p className="py-10 text-center text-sub">{dict.forums.noTopics}</p>
      ) : (
        <table className="nexus-table">
          <thead>
            <tr>
              <td className="colhead">{dict.forums.topicTitleFallback}</td>
              <td className="colhead w-32">{dict.forums.author}</td>
              <td className="colhead w-16 text-right">
                {dict.forums.replies.replace("{n}", "").trim() ||
                  dict.forums.replies}
              </td>
              <td className="colhead hidden w-20 text-right sm:table-cell">
                {dict.forums.views.replace("{n}", "").trim() ||
                  dict.forums.views}
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
                    <span
                      aria-label="精华"
                      className="mr-1 text-[var(--baozi-orange-dark)]"
                    >
                      ⭐
                    </span>
                  )}
                  {t.sticky && (
                    <Icon
                      name="pin"
                      size={13}
                      className="mr-1 inline align-[-2px] font-bold text-coral"
                      title="置顶"
                    />
                  )}
                  {t.locked && (
                    <Icon
                      name="lock"
                      size={13}
                      className="mr-1 inline align-[-2px]"
                      title="已锁定"
                    />
                  )}
                  <TypeBadge
                    type={t.topic_type}
                    label={
                      t.topic_type
                        ? (dict.forums.types as Record<string, string>)[
                            t.topic_type
                          ]
                        : undefined
                    }
                    className="mr-1 align-middle"
                  />
                  {t.tags?.map((tg) => (
                    <TagChip
                      key={tg.id}
                      tag={tg}
                      className="mr-1 align-middle"
                    />
                  ))}
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
