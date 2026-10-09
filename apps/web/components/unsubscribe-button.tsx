"use client";

import { useState } from "react";
import { useRouter } from "next/navigation";
import { api, ApiError } from "@/lib/api-client";
import { useI18n } from "@/i18n/client";

/** 追更中心行内退订（0075 组订阅）：POST /torrents/groups/{gid}/unsubscribe
 *  成功后 refresh() 让 RSC 重取列表，行自然消失。 */
export function UnsubscribeButton({ groupId }: { groupId: number }) {
  const { dict } = useI18n();
  const t = dict.subscriptions;
  const router = useRouter();
  const [busy, setBusy] = useState(false);

  async function go() {
    if (busy) return;
    setBusy(true);
    try {
      await api.post(`/api/v1/torrents/groups/${groupId}/unsubscribe`, {});
      router.refresh();
    } catch (e) {
      if (e instanceof ApiError) window.alert(e.message);
    } finally {
      setBusy(false);
    }
  }

  return (
    <button
      type="button"
      onClick={go}
      disabled={busy}
      className="shrink-0 rounded-full border border-line px-3 py-0.5
        text-xs text-sub transition-colors hover:border-sky
        hover:text-sky disabled:opacity-50"
    >
      {t.unsubscribe}
    </button>
  );
}
