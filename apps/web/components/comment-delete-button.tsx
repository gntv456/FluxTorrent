"use client";

import { useState } from "react";
import { api, ApiError } from "@/lib/api-client";
import { useI18n } from "@/i18n/client";
import { useConfirm, AskDialog } from "@/components/ui/use-confirm";

/** 评论删除（staff / 作者本人）：DELETE /torrents/{id}/comments/{cid}
 *  按钮常显，无权限时靠后端 403 兜底提示（详情页 RSC 拿不到 class，不做前端判断） */
export function CommentDeleteButton({
  torrentId,
  commentId,
}: {
  torrentId: number;
  commentId: number;
}) {
  const { dict } = useI18n();
  const t = dict.commentdel;
  const [busy, setBusy] = useState(false);
  const [msg, setMsg] = useState<string | null>(null);
  const [gone, setGone] = useState(false);
  // P2 清偿：删除二次确认走 ConfirmDialog（danger），window.confirm 退役
  const ask = useConfirm();

  async function del() {
    if (busy) return;
    setBusy(true);
    setMsg(null);
    try {
      await api.del(`/api/v1/torrents/${torrentId}/comments/${commentId}`);
      setGone(true);
    } catch (e) {
      setMsg(e instanceof ApiError ? e.message : t.failed);
    } finally {
      setBusy(false);
    }
  }

  if (gone) return <span className="text-[11px] text-sub">{t.ok}</span>;
  return (
    <span className="inline-flex flex-col items-start gap-0.5">
      <button
        type="button"
        onClick={() =>
          ask.open({
            title: t.btn,
            message: t.confirm,
            tone: "danger",
            busy,
            run: del,
          })
        }
        disabled={busy}
        className="min-h-[26px] rounded-full border border-[var(--danger-border)] px-2 text-[11px] font-bold text-danger disabled:opacity-50"
      >
        🗑 {t.btn}
      </button>
      {msg && (
        <span className="text-[11px] text-danger" role="status">
          {msg}
        </span>
      )}
      <AskDialog {...ask} />
    </span>
  );
}
