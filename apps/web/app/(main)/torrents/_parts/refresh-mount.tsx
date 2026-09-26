"use client";

import { useRouter } from "next/navigation";
import { PullToRefresh } from "@/components/pull-to-refresh";
import { useI18n } from "@/i18n/client";

/** 列表页下拉刷新挂点（M3）：触屏态渲染，onRefresh 走 router.refresh()
 *  （RSC 重取，保留 URL 筛选态）。 */
export function TorrentsPullRefresh() {
  const router = useRouter();
  const { dict } = useI18n();
  return (
    <PullToRefresh
      onRefresh={() => router.refresh()}
      label={dict.common.loading}
    />
  );
}
