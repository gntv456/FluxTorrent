"use client";

import { useEffect, useState } from "react";
import { api, ApiError, hasSessionCookie } from "@/lib/api-client";
import { apiErrorMessage, useI18n } from "@/i18n/client";
import { fmt } from "@/i18n/config";
import { toast } from "@/components/ui/toast";
import {
  REPLY_EVENT,
  type ReplyTarget,
} from "@/components/comment-reply-button";

/** 种子详情页互动区（M07 社区互动；M6.5 按本地完善版样图拆两件）：
 *  - ThankButton：「感谢发布者」单钮，常驻头部行 2（样图头部四钮之一）
 *  - TorrentSocial：收藏 + 评论表单 + 魔力答谢组，收进「更多操作」
 *    面板（全断点同一面板——桌面不再内联铺开，样图头部只有
 *    下载/心愿单/感谢/更多 四钮）
 *  0156 嵌套回复：监听回复按钮的 flux:comment-reply 事件设置目标，
 *  发表时随 body 带 parent_id 提交，提交/取消后自清
 *  W-17 乐观更新：收藏按钮点击瞬间 UI 翻转（不等网络），失败时
 *  翻回 + toast.optimistic 自动改口成错误提示（含 request_id） */

/** 「感谢发布者」单钮（plain thank；带金额的打赏组在面板内 TorrentSocial） */
export function ThankButton({ torrentId }: { torrentId: number }) {
  const { dict, currency } = useI18n();
  const t = dict.tdetail;
  const [thanked, setThanked] = useState(false);
  const [busy, setBusy] = useState(false);
  const [msg, setMsg] = useState<string | null>(null);

  async function thank() {
    setBusy(true);
    try {
      const r = await api.post<{ spark_given: number }>(
        `/api/v1/torrents/${torrentId}/thanks`,
        { amount: 0 },
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
          ? (apiErrorMessage(dict, e))
          : dict.common.networkError,
      );
    } finally {
      setBusy(false);
    }
  }

  return (
    <span className="inline-flex flex-col items-start">
      <button
        onClick={thank}
        disabled={thanked || busy}
        className="min-h-[44px] rounded-full border border-line px-5 text-sm font-bold text-ink transition-transform active:scale-[0.97] disabled:opacity-60"
      >
        {thanked ? dict.torrent.thanked : dict.torrent.thanks}
      </button>
      {msg && (
        <span className="mt-1 text-xs text-sub" role="status">
          {msg}
        </span>
      )}
    </span>
  );
}

/** 收藏 + 评论表单 + 魔力答谢（进「更多操作」面板；感谢单钮在头部） */
export function TorrentSocial({ torrentId }: { torrentId: number }) {
  const { dict, currency } = useI18n();
  const t = dict.tdetail;
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

  /** 魔力答谢（同类站口径按钮组 +1..+10000）；plain thank 在头部 ThankButton */
  async function tip(amount: number) {
    setBusy(true);
    try {
      const r = await api.post<{ spark_given: number }>(
        `/api/v1/torrents/${torrentId}/thanks`,
        { amount },
      );
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
          ? (apiErrorMessage(dict, e))
          : dict.common.networkError,
      );
    } finally {
      setBusy(false);
    }
  }

  async function toggleBookmark() {
    // W-17 乐观更新：先翻转 UI + 成功态 toast，失败由 optimistic 改口
    // （错误 toast 带 request_id），UI 翻回由返回值驱动
    const on = !bookmarked;
    setBookmarked(on);
    const ok = await toast.optimistic(
      on ? dict.torrent.bookmarkOk : dict.torrent.unbookmarkOk,
      {
        run: () =>
          api.put(`/api/v1/torrents/${torrentId}/bookmark`, { on }),
        rollbackMessage: dict.ui.optimisticRollback,
      },
    );
    if (!ok) setBookmarked(!on);
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
          ? (apiErrorMessage(dict, e))
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

      {/* 魔力答谢（同类站口径按钮组） */}
      <div className="donate-spark">
        <span className="text-xs font-bold text-sub">
          {/* 0173：{magic} 占位符须替换为站点货币名，不得原样渲染 */}
          {fmt(t.sparkReward, { magic: currency })}
        </span>
        <div className="flex flex-wrap gap-2">
          {[1, 10, 100, 500, 1000, 10000].map((v) => (
            <button
              key={v}
              onClick={() => tip(v)}
              disabled={busy}
              className="min-h-[32px] rounded-full border border-[var(--brand-orange)] px-3 text-xs font-bold text-[var(--brand-orange-dark)] active:scale-[0.97] disabled:opacity-50"
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
