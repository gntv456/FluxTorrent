"use client";

import nextDynamic from "next/dynamic";

/** 列表 hover 预览懒壳（2026-11-11 提速批）：事件委托 island，首悬
 *  400ms 才可能出卡——拖到悬停发生再下载毫无感知（RSC 里 ssr 不可关，
 *  组件挂载即绑委托监听，SSR 空壳无碍）。 */
export const TorrentHoverMount = nextDynamic(
  () =>
    import("@/components/torrent-hover-preview").then(
      (m) => m.TorrentHoverPreview,
    ),
  { loading: () => null },
);
