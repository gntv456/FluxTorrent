"use client";

import { useState } from "react";
import { useRouter } from "next/navigation";
import { api, ApiError } from "@/lib/api-client";
import { useI18n } from "@/i18n/client";
import { toast } from "@/components/ui/toast";

/** 追更中心「我的收藏」行内取消收藏：PUT /torrents/{id}/bookmark {on:false}
 *  （与种子详情页的收藏按钮同一写口，不另起端点）。
 *  成功后 refresh() 让 RSC 重取列表，行自然消失。 */
export function BookmarkRemoveButton({
  torrentId,
}: {
  torrentId: number;
}) {
  const { dict } = useI18n();
  const router = useRouter();
  const [busy, setBusy] = useState(false);

  async function go() {
    if (busy) return;
    setBusy(true);
    try {
      await api.put(`/api/v1/torrents/${torrentId}/bookmark`, { on: false });
      toast.success(dict.torrent.unbookmarkOk);
      router.refresh();
    } catch (e) {
      if (e instanceof ApiError) {
        toast.error(e.message, { requestId: e.requestId });
      } else {
        toast.error(dict.common.networkError);
      }
      setBusy(false);
    }
  }

  return (
    <button
      type="button"
      onClick={go}
      disabled={busy}
      className="shrink-0 rounded-full border border-line px-3 py-0.5
        text-xs text-sub transition-colors
        hover:border-[var(--sky)] hover:text-[var(--sky)]
        disabled:opacity-50"
    >
      {busy ? dict.ui.actionFeedbackBusy : dict.my.bmRemove}
    </button>
  );
}
