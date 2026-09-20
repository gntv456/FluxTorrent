"use client";

import { useState } from "react";
import { useRouter } from "next/navigation";
import { api, ApiError } from "@/lib/api-client";
import { useI18n } from "@/i18n/client";

/** 帖子点赞条（0116）：乐观更新，失败回滚。颜色取主题 token，明暗自适应。 */
export function PostVoteBar({
  postId,
  likes,
  liked,
}: {
  postId: number;
  likes: number;
  liked: boolean;
}) {
  const { dict } = useI18n();
  const [on, setOn] = useState(liked);
  const [n, setN] = useState(likes);
  const [busy, setBusy] = useState(false);

  async function toggle() {
    if (busy) return;
    setBusy(true);
    const next = !on;
    setOn(next);
    setN((x) => x + (next ? 1 : -1));
    try {
      const r = next
        ? await api.post<{ likes: number; liked: boolean }>(`/api/v1/forums/posts/${postId}/like`)
        : await api.del<{ likes: number; liked: boolean }>(`/api/v1/forums/posts/${postId}/like`);
      if (r) {
        setOn(r.liked);
        setN(r.likes);
      }
    } catch {
      setOn(!next);
      setN((x) => x + (next ? -1 : 1));
    } finally {
      setBusy(false);
    }
  }

  return (
    <button
      type="button"
      onClick={toggle}
      disabled={busy}
      aria-pressed={on}
      title={dict.forums.like}
      className={`inline-flex min-h-[28px] items-center gap-1 rounded-full border px-2.5 text-xs font-bold transition disabled:opacity-50 ${
        on
          ? "border-transparent bg-[var(--coral-soft)] text-coral"
          : "border-line text-sub hover:border-coral hover:text-coral"
      }`}
    >
      <span aria-hidden="true">{on ? "❤️" : "🤍"}</span>
      <span className="num">{n > 0 ? n : dict.forums.like}</span>
    </button>
  );
}

/** 主题收藏按钮（0116）：乐观更新。 */
export function TopicFavoriteButton({
  topicId,
  faved,
  favorites,
}: {
  topicId: number;
  faved: boolean;
  favorites: number;
}) {
  const { dict } = useI18n();
  const [on, setOn] = useState(faved);
  const [n, setN] = useState(favorites);
  const [busy, setBusy] = useState(false);

  async function toggle() {
    if (busy) return;
    setBusy(true);
    const next = !on;
    setOn(next);
    setN((x) => x + (next ? 1 : -1));
    try {
      const r = next
        ? await api.post<{ favorites: number; faved: boolean }>(`/api/v1/forums/topics/${topicId}/favorite`)
        : await api.del<{ favorites: number; faved: boolean }>(`/api/v1/forums/topics/${topicId}/favorite`);
      if (r) {
        setOn(r.faved);
        setN(r.favorites);
      }
    } catch {
      setOn(!next);
      setN((x) => x + (next ? -1 : 1));
    } finally {
      setBusy(false);
    }
  }

  return (
    <button
      type="button"
      onClick={toggle}
      disabled={busy}
      aria-pressed={on}
      title={dict.forums.favorite}
      className={`inline-flex min-h-[32px] items-center gap-1 rounded-full border px-3 text-xs font-bold transition disabled:opacity-50 ${
        on
          ? "border-transparent bg-[var(--coral-soft)] text-coral"
          : "border-line text-sub hover:border-coral hover:text-coral"
      }`}
    >
      <span aria-hidden="true">{on ? "⭐" : "☆"}</span>
      {on ? dict.forums.favorited : dict.forums.favorite}
      {n > 0 && <span className="num text-sub">· {n}</span>}
    </button>
  );
}

/** 打赏按钮（0127）：楼层上「请作者喝一杯」，弹 prompt 输金额；对冲转账（spend+earn 同额）。
 *  只对非本人的楼渲染（自己转自己是流水噪音）；登录由父层 authId>0 保证。 */
export function PostTipButton({
  postId,
  tips,
  tipCount,
  currency,
}: {
  postId: number;
  tips: number;
  tipCount: number;
  currency: string;
}) {
  const { dict } = useI18n();
  const [total, setTotal] = useState(tips);
  const [n, setN] = useState(tipCount);
  const [busy, setBusy] = useState(false);

  async function tip() {
    if (busy) return;
    const raw = window.prompt(dict.forums.tipPrompt.replace("{magic}", currency));
    if (!raw) return;
    const amt = Math.floor(Number(raw));
    if (!Number.isFinite(amt) || amt <= 0) {
      window.alert(dict.forums.tipInvalid);
      return;
    }
    setBusy(true);
    try {
      await api.post("/api/v1/forums/tip", { post_id: postId, spark: amt });
      setTotal((t) => t + amt);
      setN((x) => x + 1);
    } catch (e) {
      window.alert(e instanceof ApiError ? e.message : dict.common.networkError);
    } finally {
      setBusy(false);
    }
  }

  return (
    <button
      type="button"
      onClick={tip}
      disabled={busy}
      title={dict.forums.tipTitle}
      className="inline-flex min-h-[28px] items-center gap-1 rounded-full border px-2.5 text-xs font-bold transition disabled:opacity-50 border-line text-sub hover:border-[var(--baozi-orange)] hover:text-[var(--baozi-orange-dark)]"
    >
      <span aria-hidden="true">☕</span>
      {dict.forums.tipTitle}
      {n > 0 && (
        <span className="num opacity-80">
          {total} {currency} · {n}
        </span>
      )}
    </button>
  );
}

/** 悬赏采纳按钮（0124）：楼主在任意回复楼层上点「采纳」，赏金发放给答主后整页刷新。 */
export function BountyAcceptButton({
  topicId,
  postId,
  spark,
  currency,
}: {
  topicId: number;
  postId: number;
  spark: number;
  currency: string;
}) {
  const { dict } = useI18n();
  const router = useRouter();
  const [busy, setBusy] = useState(false);

  async function accept() {
    if (busy) return;
    if (!window.confirm(dict.forums.bountyConfirm.replace("{n}", String(spark)))) return;
    setBusy(true);
    try {
      await api.post("/api/v1/forums/bounty/award", { topic_id: topicId, post_id: postId });
      router.refresh();
    } catch {
      setBusy(false);
    }
  }

  return (
    <button
      type="button"
      onClick={accept}
      disabled={busy}
      title={dict.forums.bountyAccept}
      className="inline-flex min-h-[30px] items-center gap-1 rounded-full border border-coral bg-transparent px-3 text-xs font-bold text-coral transition hover:bg-[var(--coral-soft)] disabled:opacity-50"
    >
      <span aria-hidden="true">💰</span>
      {dict.forums.bountyAccept}
      <span className="num opacity-80">
        +{spark} {currency}
      </span>
    </button>
  );
}
