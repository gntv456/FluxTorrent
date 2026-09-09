"use client";

import { useEffect, useState } from "react";
import { api, ApiError } from "@/lib/api-client";
import { useI18n } from "@/i18n/client";

/** 感谢 / 收藏 / 评论表单（M07 社区互动，客户端叶子组件） */
export function TorrentSocial({ torrentId }: { torrentId: number }) {
  const { dict } = useI18n();
  const [thanked, setThanked] = useState(false);
  const [bookmarked, setBookmarked] = useState(false);
  const [comment, setComment] = useState("");
  const [msg, setMsg] = useState<string | null>(null);
  const [busy, setBusy] = useState(false);
  const [hasToken, setHasToken] = useState(false);

  useEffect(() => {
    setHasToken(Boolean(localStorage.getItem("flux.token")));
  }, []);

  async function thank() {
    try {
      await api.post(`/api/v1/torrents/${torrentId}/thanks`, {});
      setThanked(true);
      setMsg(dict.torrent.thanksOk);
    } catch (e) {
      setMsg(
        e instanceof ApiError ? (dict.errors[e.code] ?? e.message) : dict.common.networkError,
      );
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
        e instanceof ApiError ? (dict.errors[e.code] ?? e.message) : dict.common.networkError,
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
      });
      setMsg(dict.torrent.commentOk);
      setComment("");
      // RSC 页面整页刷新以带出最新评论列表
      setTimeout(() => location.reload(), 600);
    } catch (e) {
      setMsg(
        e instanceof ApiError ? (dict.errors[e.code] ?? e.message) : dict.common.networkError,
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
          onClick={thank}
          disabled={thanked}
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
          <input
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
      {msg && (
        <p role="status" className="text-sm text-sub">
          {msg}
        </p>
      )}
    </div>
  );
}
