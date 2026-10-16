"use client";

import { useState } from "react";
import type { Dict } from "@/i18n/server";
import type { TorrentComment } from "@fluxtorrent/domain-types";

/** 评论区「加载更多」（聚合审计 P2-3，从 torrent-detail-blocks 拆出：
 *  300 行门禁）。聚合口首屏固定 20 条，本组件按 limit 递增分页拉取
 *  /torrents/{id}/comments；调用侧把 total（torrent.comments 口径）
 *  传入以判断余量与「已显示全部」尾巴。 */
export function CommentsLoadMore({
  comments,
  setComments,
  torrentId,
  total,
  dict,
}: {
  comments: TorrentComment[];
  setComments: (next: TorrentComment[]) => void;
  torrentId: number;
  total: number;
  dict: Dict;
}) {
  const [busy, setBusy] = useState(false);
  const remaining = total - comments.length;
  // 种子被删评/新评后 total 与已加载条数可能错位：只在明确还有余量时出按钮
  if (remaining <= 0) {
    if (total > 20) {
      return <p className="td-comments__end">{dict.torrent.commentsEnd}</p>;
    }
    return null;
  }
  const loadMore = async () => {
    if (busy) return;
    setBusy(true);
    try {
      const { api } = await import("@/lib/api-client");
      const next = await api.get<TorrentComment[]>(
        `/api/v1/torrents/${torrentId}/comments?limit=${comments.length + 20}`,
      );
      setComments(next);
    } catch {
      /* 失败保持现状，按钮可重试 */
    } finally {
      setBusy(false);
    }
  };
  return (
    <button
      type="button"
      className="td-comments__more"
      onClick={loadMore}
      disabled={busy}
    >
      {busy
        ? dict.torrent.commentBusy
        : dict.torrent.commentsMore.replace("{n}", String(remaining))}
    </button>
  );
}
