import Link from "next/link";
import { notFound } from "next/navigation";
import { getPosts, getForums, getEmbedRules } from "@/lib/data";
import { TopicModActions } from "@/components/forum-post-actions";
import { ReplyBox } from "@/components/forum-reply-box";
import { TypeBadge, TagChip } from "@/components/forum-bits";
import { Icon } from "@/components/icons";
import { TopicFavoriteButton } from "@/components/forum-vote";
import { PollWidget } from "@/components/forum-poll";
import { LotteryWidget } from "@/components/forum-lottery";
import { ReportTopicButton } from "@/components/forum-report";
import { FollowButton } from "@/components/forum-follow";
import { PostRow } from "./_parts/post-row";
import { getDict } from "@/i18n/server";
import { requireModule } from "@/components/module-gate";

export const dynamic = "force-dynamic";

export default async function TopicPage({
  params,
  searchParams,
}: {
  params: Promise<{ id: string }>;
  searchParams: Promise<{ before?: string; reply_to?: string }>;
}) {
  const gate = await requireModule("forums");
  if (gate) return gate;
  const { id } = await params;
  const { before, reply_to } = await searchParams;
  const topicId = Number(id);
  const { dict, locale, currency } = await getDict();
  if (!Number.isFinite(topicId)) notFound();
  const detail = await getPosts(topicId, before ? Number(before) : undefined);
  if (!detail || detail.posts.length === 0) notFound();
  // 版主「移动到」下拉用：仅 can_mod 时才需要
  const forums = detail.can_mod ? await getForums() : [];
  // 视频内嵌白名单规则（0189）：开关关闭/失败 → 空数组，视频语法降级链接
  const embedRules = await getEmbedRules();
  const authId = detail.current_user_id ?? -1;
  // 楼中楼（0163）：?reply_to=N 定位目标楼（含楼中楼），供回复框显示徽标
  const replyToId = Number(reply_to) || null;
  const replyTarget = replyToId
    ? (detail.posts
        .flatMap((p) => [p, ...(p.replies ?? [])])
        .find((x) => x.id === replyToId) ?? null)
    : null;

  return (
    <div className="flex flex-col gap-4">
      <div className="flex items-baseline gap-2">
        <Link href="/forums" className="text-sm text-sky">
          {dict.forums.backToForums}
        </Link>
        {detail.forum_name && (
          <Link
            href={`/forums/${detail.forum_id}`}
            className="text-sm text-sub hover:text-sky"
          >
            » {detail.forum_name}
          </Link>
        )}
      </div>
      <div className="flex flex-wrap items-center gap-2">
        <h1 className="font-display text-2xl">
          {detail.sticky && (
            <Icon
              name="pin"
              size={15}
              className="mr-1 inline align-[-3px] text-coral"
            />
          )}
          {detail.digest && (
            <Icon
              name="star"
              size={15}
              className={
                "mr-1 inline align-[-3px] text-[var(--baozi-orange-dark)]"
              }
            />
          )}
          {detail.locked && (
            <Icon name="lock" size={15} className="mr-1 inline align-[-3px]" />
          )}
          <TypeBadge
            type={detail.topic_type}
            label={
              detail.topic_type
                ? (dict.forums.types as Record<string, string>)[
                    detail.topic_type
                  ]
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
            <a
              href={`#p${detail.bounty_post_id}`}
              className="text-xs text-sky hover:underline"
            >
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
      {detail.topic_type === "poll" &&
        detail.poll &&
        detail.poll.options?.length >= 2 && (
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
            <PostRow
              key={p.id}
              p={p}
              i={i}
              detail={detail}
              authId={authId}
              dict={dict}
              locale={locale}
              currency={currency}
              embedRules={embedRules}
            />
          ))}
        </tbody>
      </table>
      {detail.locked && !detail.can_mod ? (
        <p className="rounded-[var(--r-md)] bg-sky-soft p-3 text-center text-sm text-sub">
          主题已锁定，无法回复
        </p>
      ) : detail.can_write ? (
        <ReplyBox
          topicId={topicId}
          replyTarget={
            replyTarget
              ? { id: replyTarget.id, username: replyTarget.username }
              : null
          }
        />
      ) : (
        <p className="rounded-[var(--r-md)] bg-sky-soft p-3 text-center text-sm text-sub">
          您没有在本版块回帖的权限
        </p>
      )}
    </div>
  );
}
