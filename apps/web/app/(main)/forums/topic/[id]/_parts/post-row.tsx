/**
 * 论坛楼层行（2026-09-23 从 topic/[id]/page.tsx 拆出，行数门禁 300）：
 * NP 经典双栏 —— 左栏作者卡（头像/用户名链接/等级/佩戴勋章/楼层号/入站时间），
 * 右栏正文与操作行（时间·编辑标记·中选标记·悬赏采纳·打赏·点赞·编辑删除）。
 * Server component：无 hooks，全部数据由 page.tsx 预取。
 */
import Link from "next/link";
import { MedalIcon } from "@/components/medal-icon";
import { Icon } from "@/components/icons";
import { MarkdownRenderer } from "@/components/forum-markdown";
import { PostActions } from "@/components/forum-post-actions";
import {
  PostVoteBar,
  BountyAcceptButton,
  PostTipButton,
} from "@/components/forum-vote";
import type { Post, TopicDetail } from "@fluxtorrent/domain-types";
import type { Dict } from "@/i18n/server";
import { dateLocale, fmt } from "@/i18n/config";
import type { Locale } from "@/i18n/config";

/** 长类名提常量（行宽门禁 ≤80） */
const ACTION_BAR =
  "mt-2 flex flex-wrap items-center gap-3 text-[11px] text-sub";

const REPLY_BTN =
  "inline-flex min-h-[30px] items-center rounded-full border border-line " +
  "px-3 text-xs font-bold text-sub hover:border-sky";
const REPLIES_BOX =
  "mt-2 flex flex-col gap-2 rounded-[var(--r-md)] " +
  "bg-[var(--surface-sunken)] p-3";
const MINI_AVATAR =
  "mt-0.5 flex h-7 w-7 shrink-0 items-center justify-center " +
  "rounded-[var(--r-sm)] border border-line bg-[var(--surface-card)]";
const AVATAR_BOX =
  "mb-1 flex h-14 w-14 items-center justify-center overflow-hidden " +
  "rounded-[var(--r-md)] border border-line bg-[var(--surface-sunken)]";

export function PostRow({
  p,
  i,
  detail,
  authId,
  dict,
  locale,
  currency,
}: {
  p: Post;
  i: number;
  detail: TopicDetail;
  authId: number;
  dict: Dict;
  locale: Locale;
  currency: string;
}) {
  return (
    <tr id={`p${p.id}`} className="align-top">
      <td className="w-36 border-r border-line bg-[rgba(255,232,197,0.45)] p-3">
        {/* 楼层作者栏（2026-09-23）：头像 + 用户名 + 等级 + 佩戴勋章 + 入站时间 */}
        <div className={AVATAR_BOX}>
          {p.avatar_url ? (
            // eslint-disable-next-line @next/next/no-img-element
            <img
              src={p.avatar_url}
              alt={p.username ?? ""}
              className="h-full w-full object-cover"
            />
          ) : p.username ? (
            <span className="font-display text-lg text-deep">
              {p.username.slice(0, 1)}
            </span>
          ) : (
            <Icon name="user" size={22} />
          )}
        </div>
        {p.user_id ? (
          <Link
            href={`/users/${p.user_id}`}
            className="font-bold text-sky hover:underline"
          >
            {p.username}
          </Link>
        ) : (
          <span className="font-bold text-sky">
            {p.username ?? dict.torrent.anonymous}
          </span>
        )}
        {p.class_name && (
          <p className="mt-0.5 text-[11px] font-bold text-sub">
            {p.class_name}
          </p>
        )}
        {p.worn_medals && p.worn_medals.length > 0 && (
          <div className="mt-1 flex flex-wrap gap-1">
            {p.worn_medals.slice(0, 3).map((m) => (
              <span key={m.name} className="medal-chip" title={m.name}>
                {m.asset_ref ? (
                  <MedalIcon src={m.asset_ref} size={14} title={m.name} />
                ) : (
                  <b className="text-[10px] text-ink">{m.name.slice(0, 1)}</b>
                )}
              </span>
            ))}
          </div>
        )}
        {p.author_joined_at && (
          <p className="mt-1 text-[10px] text-fainter">
            {dict.forums.joinedShort}
            {new Date(p.author_joined_at).toLocaleDateString(
              dateLocale(locale),
            )}
          </p>
        )}
      </td>
      <td className="p-3">
        <MarkdownRenderer source={p.body} />
        <div className={ACTION_BAR}>
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
            <span
              className={
                "rounded-full bg-[var(--coral-soft)] " +
                "px-2 py-0.5 font-bold text-coral"
              }
            >
              ✓ {dict.forums.bountyPicked}
            </span>
          )}
          {/* 悬赏未决 + 我是楼主 + 这层不是首帖：*/}
          {/* 显示采纳按钮 */}
          {detail.topic_type === "bounty" &&
            detail.bounty_status === "open" &&
            detail.is_op &&
            i > 0 &&
            p.user_id !== authId && (
              <BountyAcceptButton
                topicId={detail.topic_id}
                postId={p.id}
                spark={detail.bounty_spark ?? 0}
                currency={currency}
              />
            )}
          {/* 回复该楼（0163 楼中楼）：带 ?reply_to= 跳定向回复框（匿名楼不渲染） */}
          {p.user_id && (
            <Link
              href={`/forums/topic/${detail.topic_id}?reply_to=${p.id}`}
              className={REPLY_BTN}
            >
              回复
            </Link>
          )}
          {/* 编辑/删除（2026-09-23 用户要求：编辑放在赞前面）：
              PostActions 用 display:contents 融入本操作行 */}
          {(detail.can_mod || p.user_id === authId) &&
            p.body !== "……" && (
              <PostActions
                postId={p.id}
                canMod={detail.can_mod}
                isSelf={p.user_id === authId}
                initialBody={p.body}
              />
            )}
          {authId > 0 && (
            <span className="ml-3 flex items-center gap-2">
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
          {/* 楼层号（2026-09-23 用户要求：右下角 + 第 n 层文案） */}
          <span className="ml-auto font-bold">
            {fmt(dict.forums.floor, { n: i + 1 })}
          </span>
        </div>
        {/* 楼中楼（0163）：挂在所属顶层楼下的回复，缩进浅底展示 */}
        {p.replies && p.replies.length > 0 && (
          <div className={REPLIES_BOX}>
            {p.replies.map((r) => (
              <div
                key={r.id}
                id={`p${r.id}`}
                className="flex items-start gap-2"
              >
                <span className={MINI_AVATAR}>
                  {r.avatar_url ? (
                    // eslint-disable-next-line @next/next/no-img-element
                    <img
                      src={r.avatar_url}
                      alt={r.username ?? ""}
                      className="h-full w-full object-cover"
                    />
                  ) : (
                    <span className="text-[11px] font-bold text-deep">
                      {(r.username ?? "?").slice(0, 1)}
                    </span>
                  )}
                </span>
                <div className="min-w-0 flex-1">
                  <p className="text-[11px] leading-5">
                    {r.user_id ? (
                      <Link
                        href={`/users/${r.user_id}`}
                        className="font-bold text-sky hover:underline"
                      >
                        {r.username}
                      </Link>
                    ) : (
                      <b>{r.username ?? dict.torrent.anonymous}</b>
                    )}
                    {r.reply_to_name && (
                      <span className="text-sub"> 回复 @{r.reply_to_name}</span>
                    )}
                  </p>
                  <MarkdownRenderer source={r.body} />
                  <p className="text-[10px] text-fainter">
                    {new Date(r.created_at).toLocaleString(dateLocale(locale))}
                  </p>
                </div>
              </div>
            ))}
          </div>
        )}
      </td>
    </tr>
  );
}
