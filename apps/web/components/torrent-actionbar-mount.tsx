"use client";

import { TorrentMobileActionBar } from "@/components/torrent-mobile-actionbar";

/** 详情页移动操作条客户端壳（M3）：把桌面 DownloadButton 的下载逻辑
 *  （blob 保存，含付费扣费提示）与收藏/复制链接触到 44px 底条上。
 *  下载复用 /api/v1/torrents/{id}/download 的 blob 流（与桌面同一接口、
 *  同一 Bearer 凭证）；复制链接取 location.href。 */
export function TorrentActionBarMount({
  torrentId,
  name,
  price,
  labels,
}: {
  torrentId: number;
  name: string;
  price?: number;
  labels: { fav: string; copy: string; download: string };
}) {
  const download = async () => {
    const { api, ApiError } = await import("@/lib/api-client");
    try {
      const data = await api.getBlob(
        `/api/v1/torrents/${torrentId}/download`,
      );
      const blob = new Blob([data], { type: "application/x-bittorrent" });
      const url = URL.createObjectURL(blob);
      const a = document.createElement("a");
      a.href = url;
      a.download = `${name}.torrent`;
      a.click();
      URL.revokeObjectURL(url);
    } catch (e) {
      if (e instanceof ApiError) alert(e.message);
    }
  };
  const copy = async () => {
    try {
      await navigator.clipboard.writeText(window.location.href);
    } catch {
      /* 剪贴板权限拒绝时静默（HTTP 环境常见） */
    }
  };
  const favorite = () => {
    // 收藏走心愿单关键词接口（与 WishlistButton 同源）；移动端复用
    // 桌面按钮所在行为——这里直接触发同接口的轻量封装
    import("@/lib/api-client").then(({ api }) => {
      api
        .post("/api/v1/wishlist", { keyword: name })
        .catch(() => undefined);
    });
  };
  return (
    <TorrentMobileActionBar
      torrentId={torrentId}
      onDownload={download}
      onFavorite={favorite}
      onCopyLink={copy}
      labels={labels}
      key={price ?? 0}
    />
  );
}
