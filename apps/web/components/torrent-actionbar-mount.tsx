"use client";

import { TorrentMobileActionBar } from "@/components/torrent-mobile-actionbar";
import { toast } from "@/components/ui/toast";
import { useI18n } from "@/i18n/client";

/** 详情页移动操作条客户端壳（M3）：把桌面 DownloadButton 的下载逻辑
 *  （blob 保存，含付费扣费提示）与收藏/复制链接触到 44px 底条上。
 *  下载复用 /api/v1/torrents/{id}/download 的 blob 流（与桌面同一接口、
 *  同一 Bearer 凭证）；复制链接取 location.href。
 *  反馈通道（W-21）：失败走 toast（带 request_id 芯片），收藏成功
 *  走 toast 确认——此前 alert / 静默均不符合反馈协议。 */

/** 下载失败 toast 文案（字典优先，ApiError 消息更具体时透传） */
function failMsg(e: unknown, fallback: string): string {
  return e instanceof Error && e.message ? e.message : fallback;
}

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
  const { dict } = useI18n();
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
      // W-21：裸 alert → toast.error（request_id 芯片可复制报障）
      toast.error(
        e instanceof ApiError
          ? e.message
          : failMsg(e, dict.common.networkError),
        { requestId: e instanceof ApiError ? e.requestId : undefined },
      );
    }
  };
  const copy = async () => {
    try {
      await navigator.clipboard.writeText(window.location.href);
      toast.success(dict.torrent.linkCopied);
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
        .then(() => toast.success(dict.wishlist.addedOk))
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
