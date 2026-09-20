import Link from "next/link";
import { notFound } from "next/navigation";
import { getPosts, getForums } from "@/lib/data";
import { ReplyBox, TopicModActions, PostActions } from "@/components/forum-post-actions";
import { MarkdownRenderer } from "@/components/forum-markdown";
import { TypeBadge, TagChip } from "@/components/forum-bits";
import { PostVoteBar, TopicFavoriteButton, BountyAcceptButton, PostTipButton } from "@/components/forum-vote";
import { PollWidget } from "@/components/forum-poll";
import { LotteryWidget } from "@/components/forum-lottery";
import { ReportTopicButton } from "@/components/forum-report";
import { FollowButton } from "@/components/forum-follow";
import { getDict } from "@/i18n/server";
import { dateLocale, fmt } from "@/i18n/config";

export const dynamic = "force-dynamic";

export default async function TopicPage({
  params,
  searchParams,
}: {
  params: Promise<{ id: string }>;
  searchParams: Promise<{ before?: string }>;
}) {
  const { id } = await params;
  const { before } = await searchParams;
  const topicId = Number(id);
  const { dict, locale, currency } = await getDict();
  if (!Number.isFinite(topicId)) notFound();
  const detail = await getPosts(topicId, before ? Number(before) : undefined);
  if (!detail || detail.posts.length === 0) notFound();
  // 版主「移动到」下拉用：仅 can_mod 时才需要
  const forums = detail.can_mod ? await getForums() : [];
  const authId = detail.current_user_id ?? -1;

  return (
    <div className="flex flex-col gap-4">
      <div className="flex items-baseline gap-2">
        <Link href="/forums" className="text-sm text-sky">
          {dict.forums.backToForums}
        </Link>
        {detail.forum_name && (
          <Link href={`/forums/${detail.forum_id}`} className="text-sm text-sub hover:text-sky">
            » {detail.forum_name}
          </Link>
        )}
      </div>
      <div className="flex flex-wrap items-center gap-2">
        <h1 className="font-display text-2xl">
          {detail.sticky && <span className="mr-1 text-coral">📌</span>}
          {detail.digest && <span className="mr-1 text-[var(--baozi-orange-dark)]">⭐</span>}
          {detail.locked && <span className="mr-1">🔒</span>}
          <TypeBadge
            type={detail.topic_type}
            label={
              detail.topic_type
                ? (dict.forums.types as Record<string, string>)[detail.topic_type]
                : undefined
            }
            className="mr-2 align-middle"
          />
          {detail.tags?.map((tg) => (
            <TagChip key={tg.id} tag={tg} className="mr-1 align-middle" />
          ))}
          {detail.title}
        </h1>
        {authId > 0 && (
          <>
            <FollowButton targetType="topic" targetId={topicId} showCount />
            <TopicFavoriteButton
              topicId={topicId}
              faved={detail.faved ?? false}
              favorites={detail.favorites ?? 0}
            />
            <ReportTopicButton topicId={topicId} />
          </>
        )}
      </div>
      {/* 悬赏卡（0124）：类型为 bounty 时展示赏金与状态；已采纳时点出中选楼层 */}
      {detail.topic_type === "bounty" && (detail.bounty_spark ?? 0) > 0 && (
        <div
          className={`flex flex-wrap items-center gap-2 rounded-[var(--r-md)] border p-3 text-sm ${
            detail.bounty_status === "open"
              ? "border-coral bg-[var(--coral-soft)]"
              : "border-line bg-[var(--surface-card)]"
          }`}
        >
          <span aria-hidden className="text-lg">
            💰
          </span>
          <span className="font-bold text-ink">
            {detail.bounty_status === "open"
              ? dict.forums.bountyOpen
              : detail.bounty_status === "awarded"
                ? dict.forums.bountyAwarded
                : dict.forums.bountyRefunded}
          </span>
          <span className="num font-bold text-coral">
            {detail.bounty_spark} {currency}
          </span>
          {detail.bounty_status === "awarded" && detail.bounty_post_id && (
            <a href={`#p${detail.bounty_post_id}`} className="text-xs text-sky hover:underline">
              {dict.forums.bountyGoto}
            </a>
          )}
        </div>
      )}
      {detail.can_mod && (
        <TopicModActions
          topicId={topicId}
          sticky={detail.sticky}
          locked={detail.locked}
          digest={detail.digest ?? false}
          forums={forums.map((f) => ({ id: f.id, name: f.name }))}
        />
      )}
      {/* 投票挂件（0125）：poll 类型且有选项数据时渲染；楼主/版主可截止 */}
      {detail.topic_type === "poll" && detail.poll && detail.poll.options?.length >= 2 && (
        <PollWidget
          topicId={topicId}
          poll={detail.poll}
          canClose={detail.is_op || detail.can_mod}
        />
      )}
      {/* 抽奖挂件（0126）：lottery 类型且有数据时渲染；楼主/版主可提前开奖 */}
      {detail.topic_type === "lottery" && detail.lottery && (
        <LotteryWidget
          topicId={topicId}
          lottery={detail.lottery}
          currency={currency}
          canDraw={detail.is_op || detail.can_mod}
          isOp={detail.is_op}
        />
      )}
      {/* 长帖游标（NP 分页口径）：帖子按 id < before 取上一窗口，链接翻页保 SSR 简单可靠 */}
      {detail.has_more && detail.posts[0] && (
        <Link
          href={`/forums/topic/${topicId}?before=${detail.posts[0].id}`}
          className="rounded-[var(--r-md)] border border-line bg-[var(--surface-card)] px-4 py-2 text-center text-sm text-sky hover:border-sky"
        >
          {dict.forums.loadEarlier}
        </Link>
      )}
      <table className="nexus-table">
        <tbody>
          {detail.posts.map((p, i) => (
            <tr key={p.id} id={`p${p.id}`} className="align-top">
              <td className="w-36 border-r border-line bg-[rgba(255,232,197,0.45)] p-3">
                <span className="font-bold text-sky">
                  {p.username ?? dict.torrent.anonymous}
                </span>
                <p className="mt-1 text-[11px] text-sub">
                  {fmt(dict.forums.floor, { n: i + 1 })}
                </p>
              </td>
              <td className="p-3">
                <MarkdownRenderer source={p.body} />
                <div className="mt-2 flex flex-wrap items-center gap-3 text-[11px] text-sub">
                  <span>
                    {new Date(p.created_at).toLocaleString(dateLocale(locale))}
                  </span>
                  {p.edited_at && (
                    <span className="italic">
                      已由 #{p.edited_by} 编辑于{" "}
                      {new Date(p.edited_at).toLocaleString(dateLocale(locale))}
                    </span>
                  )}
                  {/* 中选楼层标记（0124）：楼主采纳的回复 */}
                  {detail.bounty_post_id === p.id && (
                    <span className="rounded-full bg-[var(--coral-soft)] px-2 py-0.5 font-bold text-coral">
                      ✓ {dict.forums.bountyPicked}
                    </span>
                  )}
                  {/* 悬赏未决 + 我是楼主 + 这层不是首帖：显示采纳按钮 */}
                  {detail.topic_type === "bounty" &&
                    detail.bounty_status === "open" &&
                    detail.is_op &&
                    i > 0 &&
                    p.user_id !== authId && (
                      <BountyAcceptButton
                        topicId={topicId}
                        postId={p.id}
                        spark={detail.bounty_spark ?? 0}
                        currency={currency}
                      />
                    )}
                  {authId > 0 && (
                    <span className="ml-auto flex items-center gap-2">
                      {/* 打赏（0127）：非本人的楼都可打赏（匿名楼 user_id 为空不渲染） */}
                      {p.user_id && p.user_id !== authId && (
                        <PostTipButton
                          postId={p.id}
                          tips={p.tips ?? 0}
                          tipCount={p.tip_count ?? 0}
                          currency={currency}
                        />
                      )}
                      <PostVoteBar
                        postId={p.id}
                        likes={p.likes ?? 0}
                        liked={p.liked_by_me ?? false}
                      />
                    </span>
                  )}
                </div>
                {(detail.can_mod || p.user_id === authId) && p.body !== "……" && (
                  <PostActions
                    postId={p.id}
                    canMod={detail.can_mod}
                    isSelf={p.user_id === authId}
                    initialBody={p.body}
                  />
                )}
              </td>
            </tr>
          ))}
        </tbody>
      </table>
      {detail.locked && !detail.can_mod ? (
        <p className="rounded-[var(--r-md)] bg-sky-soft p-3 text-center text-sm text-sub">
          主题已锁定，无法回复
        </p>
      ) : detail.can_write ? (
        <ReplyBox topicId={topicId} />
      ) : (
        <p className="rounded-[var(--r-md)] bg-sky-soft p-3 text-center text-sm text-sub">
          您没有在本版块回帖的权限
        </p>
      )}
    </div>
  );
}
