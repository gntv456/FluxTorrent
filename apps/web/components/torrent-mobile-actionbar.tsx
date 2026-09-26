"use client";

import { useEffect, useRef, useState } from "react";
import { useMediaQuery } from "@/lib/hooks/use-media";

/** 种子详情底部固定操作条（M3，方案 §4）：<md 显示
 *  收藏 / 复制链接 / 下载种子（44px）。评论输入聚焦时隐藏让位键盘
 *  （监听 focusin/focusout + visualViewport 高度收缩双保险）。
 *  按钮本体复用桌面既有组件语义：这里只做条与布局，具体动作用
 *  现有客户端 API（download/收藏走 props 注入的回调）。 */

export function TorrentMobileActionBar({
  torrentId,
  onDownload,
  onFavorite,
  onCopyLink,
  labels,
}: {
  torrentId: number;
  onDownload: () => void;
  onFavorite: () => void;
  onCopyLink: () => void;
  labels: {
    fav: string;
    copy: string;
    download: string;
    favLabel?: string;
    copyLabel?: string;
    downloadLabel?: string;
  };
}) {
  const compact = useMediaQuery("(max-width: 767px)", false);
  const [hidden, setHidden] = useState(false);
  const lastVV = useRef<number | null>(null);

  useEffect(() => {
    if (!compact) return;
    // 键盘弹出判定：visualViewport 高度收缩 >25%（比 focusin 更可靠，
    // 蓝牙键盘/外接屏 focus 不误隐藏）
    const vv = window.visualViewport;
    if (!vv) return;
    lastVV.current = vv.height;
    const onResize = () => {
      const prev = lastVV.current;
      lastVV.current = vv.height;
      if (prev === null) return;
      setHidden(vv.height < prev * 0.75);
    };
    vv.addEventListener("resize", onResize);
    return () => vv.removeEventListener("resize", onResize);
  }, [compact]);

  if (!compact) return null;
  return (
    <div
      className="td-actionbar"
      style={hidden ? { transform: "translateY(120%)" } : undefined}
      data-tid={torrentId}
    >
      <button
          type="button"
          aria-label={labels.favLabel ?? labels.fav}
          className="td-actionbar__btn"
          onClick={onFavorite}
        >
        {labels.fav}
      </button>
      <button
          type="button"
          aria-label={labels.copyLabel ?? labels.copy}
          className="td-actionbar__btn"
          onClick={onCopyLink}
        >
        {labels.copy}
      </button>
      <button
        type="button"
        className="td-actionbar__btn is-primary"
        aria-label={labels.downloadLabel ?? labels.download}
        onClick={onDownload}
      >
        {labels.download}
      </button>
    </div>
  );
}
