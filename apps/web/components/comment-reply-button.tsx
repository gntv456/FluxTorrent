"use client";

import { useI18n } from "@/i18n/client";

/** 评论「回复」按钮（0156 嵌套回复）：点击派发 `flux:comment-reply` 自定义事件，
 *  TorrentSocial 监听该事件设置回复目标并聚焦评论框——发表时随 body 带
 *  parent_id 提交，一次提交/取消后自清。事件（而非全局状态/sessionStorage 轮询）：
 *  两个组件分处 RSC 树，事件是 React 内最小耦合的跨组件通道。 */
export const REPLY_EVENT = "flux:comment-reply";

export interface ReplyTarget {
  torrentId: number;
  /** 根楼层 id（点在回复上也是回复其根） */
  targetId: number;
  targetUser: string;
}

export function CommentReplyButton({ target }: { target: ReplyTarget }) {
  const { dict } = useI18n();

  function pick() {
    window.dispatchEvent(
      new CustomEvent<ReplyTarget>(REPLY_EVENT, { detail: target }),
    );
    const input = document.getElementById(
      "td-comment-input",
    ) as HTMLInputElement | null;
    input?.scrollIntoView({ behavior: "smooth", block: "center" });
    input?.focus();
  }

  return (
    <button
      onClick={pick}
      className="td-comment__replybtn"
      title={dict.torrent.replyTitle}
    >
      {dict.torrent.replyBtn}
    </button>
  );
}
