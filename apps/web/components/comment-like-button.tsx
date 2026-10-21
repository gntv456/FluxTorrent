"use client";

import { useState } from "react";
import { api, ApiError } from "@/lib/api-client";
import { useI18n } from "@/i18n/client";
import { toast } from "@/components/ui/toast";

/** 评论点赞按钮（0155 评论增强）：toggle 端点回 {liked, likes}；
 *  自赞/未登录由后端拒（400/401），这里就地提示不弹窗 */
export function CommentLikeButton({
  torrentId,
  commentId,
  initialLikes,
  initialLiked,
}: {
  torrentId: number;
  commentId: number;
  initialLikes: number;
  initialLiked: boolean;
}) {
  const { dict } = useI18n();
  const [likes, setLikes] = useState(initialLikes);
  const [liked, setLiked] = useState(initialLiked);
  const [busy, setBusy] = useState(false);

  async function toggle() {
    setBusy(true);
    try {
      const r = await api.post<{ liked: boolean; likes: number }>(
        `/api/v1/torrents/${torrentId}/comments/${commentId}/like`,
      );
      setLiked(r.liked);
      setLikes(r.likes);
    } catch (e) {
      // 自赞是业务校验（DomainError::Validation ⇒ code 1002）；1003 是
      // web→api 网关不通，不是这一层能就地解释的错。
      if (e instanceof ApiError && e.code === 1002) {
        // W-24 反馈协议：alert → toast.error（request_id 可诊断）
        toast.error(e.message, { requestId: e.requestId });
      }
    } finally {
      setBusy(false);
    }
  }

  return (
    <button
      onClick={toggle}
      disabled={busy}
      aria-pressed={liked}
      className={`td-comment__like ${liked ? "td-comment__like--on" : ""}`}
      title={liked ? dict.torrent.commentUnlike : dict.torrent.commentLike}
    >
      <span aria-hidden>{liked ? "♥" : "♡"}</span>
      {likes > 0 && <span className="num">{likes}</span>}
    </button>
  );
}
