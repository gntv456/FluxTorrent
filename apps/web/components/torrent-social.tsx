"use client";

import { useEffect, useState } from "react";
import { api, ApiError, hasSessionCookie } from "@/lib/api-client";
import { useI18n } from "@/i18n/client";
import { fmt } from "@/i18n/config";
import {
  REPLY_EVENT,
  type ReplyTarget,
} from "@/components/comment-reply-button";

/** 感谢 / 收藏 / 评论表单（M07 社区互动，客户端叶子组件）
 *  魔力答谢按钮组对齐馒头详情页口径（+1/+10/+100/+500/+1000/+10000）
 *  0156 嵌套回复：监听回复按钮的 flux:comment-reply 事件设置目标，
 *  发表时随 body 带 parent_id 提交，提交/取消后自清 */
export function TorrentSocial({ torrentId }: { torrentId: number }) {
  const { dict, currency } = useI18n();
  const t = dict.tdetail;
  const [thanked, setThanked] = useState(false);
  const [bookmarked, setBookmarked] = useState(false);
  const [comment, setComment] = useState("");
  const [msg, setMsg] = useState<string | null>(null);
  const [busy, setBusy] = useState(false);
  const [hasToken, setHasToken] = useState(false);
  // 回复目标（0156）：跨种子事件忽略（详情页只有一种子，防御性校验）
  const [replyTo, setReplyTo] = useState<ReplyTarget | null>(null);

  useEffect(() => {
    setHasToken(hasSessionCookie());
    const onReply = (e: Event) => {
      const d = (e as CustomEvent<ReplyTarget>).detail;
      if (d?.torrentId === torrentId) setReplyTo(d);
    };
    window.addEventListener(REPLY_EVENT, onReply);
    return () => window.removeEventListener(REPLY_EVENT, onReply);
  }, [torrentId]);

  function clearReply() {
    setReplyTo(null);
  }

  async function thank(amount = 0) {
    setBusy(true);
    try {
      const r = await api.post<{ spark_given: number }>(
        `/api/v1/torrents/${torrentId}/thanks`,
        { amount },
      );
      setThanked(true);
      setMsg(
        r.spark_given > 0
          ? t.thankSparkOk
              .replace("{n}", String(r.spark_given))
              .replace("{magic}", currency)
          : dict.torrent.thanksOk,
      );
    } catch (e) {
      setMsg(
        e instanceof ApiError
          ? (dict.errors[e.code] ?? e.message)
          : dict.common.networkError,
      );
    } finally {
      setBusy(false);
    }
  }

  async function toggleBookmark() {
    const on = !bookmarked;
    try {
      await api.put(`/api/v1/torrents/${torrentId}/bookmark`, { on });
      setBookmarked(on);
      setMsg(on ? dict.torrent.bookmarkOk : dict.torrent.unbookmarkOk);
    } catch (e) {
      setMsg(
        e instanceof ApiError
          ? (dict.errors[e.code] ?? e.message)
          : dict.common.networkError,
      );
    }
  }

  async function submitComment(e: React.FormEvent) {
    e.preventDefault();
    if (!comment.trim()) return;
    setBusy(true);
    setMsg(null);
    try {
      await api.post(`/api/v1/torrents/${torrentId}/comments`, {
        body: comment,
        parent_id: replyTo?.targetId,
      });
      setMsg(dict.torrent.commentOk);
      setComment("");
      clearReply();
      // RSC 页面整页刷新以带出最新评论列表
      setTimeout(() => location.reload(), 600);
    } catch (e) {
      setMsg(
        e instanceof ApiError
          ? (dict.errors[e.code] ?? e.message)
          : dict.common.networkError,
      );
    } finally {
      setBusy(false);
    }
  }

  if (!hasToken) {
    return <p className="text-sm text-sub">{dict.torrent.commentNoauth}</p>;
  }

  return (
    <div className="flex w-full flex-col gap-3">
      <div className="flex flex-wrap items-center gap-3">
        <button
          onClick={() => thank(0)}
          disabled={thanked || busy}
          className="min-h-[44px] rounded-full border border-line px-5 text-sm font-bold text-ink transition-transform active:scale-[0.97] disabled:opacity-60"
        >
          {thanked ? dict.torrent.thanked : dict.torrent.thanks}
        </button>
        <button
          onClick={toggleBookmark}
          className="min-h-[44px] rounded-full border border-line px-5 text-sm font-bold text-ink transition-transform active:scale-[0.97]"
        >
          {bookmarked ? dict.torrent.bookmarked : dict.torrent.bookmark}
        </button>
        <form onSubmit={submitComment} className="flex min-w-0 flex-1 gap-2">
          {replyTo && (
            <span className="td-reply-banner">
              <span>
                {dict.torrent.replyTo.replace("{user}", replyTo.targetUser)}
              </span>
              <button
                type="button"
                onClick={clearReply}
                aria-label={dict.torrent.replyCancel}
                className="td-reply-banner__x"
              >
                ×
              </button>
            </span>
          )}
          <input
            id="td-comment-input"
            type="text"
            value={comment}
            onChange={(e) => setComment(e.target.value)}
            placeholder={dict.torrent.commentPlaceholder}
            aria-label={dict.torrent.commentSubmit}
            maxLength={2000}
            className="min-h-[44px] min-w-0 flex-1 rounded-full border border-line bg-cloud px-4 text-sm outline-none focus:border-sky"
          />
          <button
            type="submit"
            disabled={busy}
            className="min-h-[44px] shrink-0 rounded-full bg-sky-deep px-5 text-sm font-bold text-white active:scale-[0.97] disabled:opacity-50"
          >
            {busy ? dict.torrent.commentBusy : dict.torrent.commentSubmit}
          </button>
        </form>
      </div>

      {/* 魔力答谢（馒头口径按钮组） */}
      <div className="donate-spark">
        <span className="text-xs font-bold text-sub">
          {/* 0173：{magic} 占位符须替换为站点货币名，不得原样渲染 */}
          {fmt(t.sparkReward, { magic: currency })}
        </span>
        <div className="flex flex-wrap gap-2">
          {[1, 10, 100, 500, 1000, 10000].map((v) => (
            <button
              key={v}
              onClick={() => thank(v)}
              disabled={busy}
              className="min-h-[32px] rounded-full border border-[var(--baozi-orange)] px-3 text-xs font-bold text-[var(--baozi-orange-dark)] active:scale-[0.97] disabled:opacity-50"
            >
              +{v}
            </button>
          ))}
        </div>
      </div>

      {msg && (
        <p role="status" className="text-sm text-sub">
          {msg}
        </p>
      )}
    </div>
  );
}
