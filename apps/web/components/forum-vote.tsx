"use client";

import { useState } from "react";
import { api } from "@/lib/api-client";
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
