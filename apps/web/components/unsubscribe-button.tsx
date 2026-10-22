"use client";

import { useState } from "react";
import { useRouter } from "next/navigation";
import { api, ApiError } from "@/lib/api-client";
import { useI18n } from "@/i18n/client";
import { toast } from "@/components/ui/toast";

/** 追更中心行内退订：资源组（0075）与厂牌/字幕组（P-588）共用。
 *  成功后 refresh() 让 RSC 重取列表，行自然消失。 */
export function UnsubscribeButton({
  groupId,
  networkId,
}: {
  groupId?: number;
  networkId?: number;
}) {
  const { dict } = useI18n();
  const t = dict.subscriptions;
  const router = useRouter();
  const [busy, setBusy] = useState(false);

  async function go() {
    if (busy) return;
    setBusy(true);
    try {
      const url =
        networkId !== undefined
          ? `/api/v1/networks/${networkId}/unsubscribe`
          : `/api/v1/torrents/groups/${groupId}/unsubscribe`;
      await api.post(url, {});
      router.refresh();
    } catch (e) {
      // P2 清偿：错误反馈 alert → toast.error（request_id 芯片）
      if (e instanceof ApiError)
        toast.error(e.message, { requestId: e.requestId });
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
