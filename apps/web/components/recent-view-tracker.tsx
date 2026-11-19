"use client";

import { useEffect } from "react";
import { pushRecentView } from "@/lib/recent-views";

/** 最近浏览采集器：挂种子详情页，只写本地、零请求、零 DOM。 */
export function RecentViewTracker({
  torrentId,
  name,
}: {
  torrentId: number;
  name: string;
}) {
  useEffect(() => {
    pushRecentView(torrentId, name);
  }, [torrentId, name]);
  return null;
}
