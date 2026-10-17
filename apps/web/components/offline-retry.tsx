"use client";

import { useState } from "react";
import { useI18n } from "@/i18n/client";

/** 离线页的「重新连接」：重新加载当前文档，Service Worker 会再探一次网络。
 *  这里不发探测请求——离线页常在 `navigator.onLine=false` 的场景被打开，
 *  真正的判据是「用户点了之后能不能回到原来那一页」，reload 是唯一
 *  不猜 URL、不猜路由的做法。 */
export function OfflineRetry() {
  const { dict } = useI18n();
  const [busy, setBusy] = useState(false);
  return (
    <button
      type="button"
      disabled={busy}
      onClick={() => {
        setBusy(true);
        window.location.reload();
      }}
      className={BTN}
    >
      {busy ? dict.offline.retryBusy : dict.offline.retry}
    </button>
  );
}

const BTN =
  "min-h-[44px] inline-flex items-center rounded-full px-6 font-bold" +
  " bg-[var(--sky-deep)] text-white disabled:opacity-60";
