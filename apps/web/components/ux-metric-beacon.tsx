"use client";
/**
 * 体验度量渲染侧触发器（C4）：uxMark 自身在无 window 时静默返回，
 * 因此真正的打点必须落在客户端小件里；本文件不承载任何 UI。
 * - EmptyStateMetric：空态一渲染就记 empty_state（label = 四态 context）
 * - TorrentOpenMetric：资源行链接点击记首个种子打开（会话内只记一次）
 */
import { useEffect } from "react";
import { uxMark } from "@/lib/ux-metrics";

export function EmptyStateMetric({ context }: { context: string }) {
  useEffect(() => {
    uxMark("empty_state", context);
  }, [context]);
  return null;
}

/** 会话内只记一次：名字即 first_torrent_open，重复打开不重复计数。 */
let firstOpenMarked = false;

export function TorrentOpenMetric() {
  useEffect(() => {
    const onClick = (event: MouseEvent) => {
      if (firstOpenMarked || event.button !== 0) return;
      const el = event.target;
      if (!(el instanceof Element)) return;
      const link = el.closest('a[href^="/torrent/"]');
      if (!link) return;
      firstOpenMarked = true;
      uxMark("first_torrent_open", link.getAttribute("href") ?? undefined);
    };
    document.addEventListener("click", onClick);
    return () => document.removeEventListener("click", onClick);
  }, []);
  return null;
}
