"use client";

import { useState } from "react";
import { api, ApiError } from "@/lib/api-client";
import { useI18n } from "@/i18n/client";

/** 下载按钮：携带 Bearer token 以 blob 触发保存（后端 require_auth）
 *  付费种子（0086）：显示价格角标；扣费/已购由后端 download 原子处理，余额不足会弹错 */
export function DownloadButton({
  torrentId,
  name,
  price,
  purchased,
  isOwner,
}: {
  torrentId: number;
  name: string;
  price?: number;
  purchased?: boolean;
  isOwner?: boolean;
}) {
  const { dict, currency } = useI18n();
  const [state, setState] = useState<"idle" | "busy" | "noauth">("idle");
  const charged = (price ?? 0) > 0 && !purchased && !isOwner;

  async function download() {
    setState("busy");
    try {
      const data = await api.getBlob(`/api/v1/torrents/${torrentId}/download`);
      const blob = new Blob([data], { type: "application/x-bittorrent" });
      const url = URL.createObjectURL(blob);
      const a = document.createElement("a");
      a.href = url;
      a.download = `${name}.torrent`;
      a.click();
      URL.revokeObjectURL(url);
      setState("idle");
    } catch (e) {
      const noauth = e instanceof ApiError && e.code === 2001;
      setState(noauth ? "noauth" : "idle");
      alert(
        noauth
          ? dict.torrent.downloadNoauth
          : e instanceof Error
            ? e.message
            : dict.torrent.downloadFailed,
      );
    }
  }

  return (
    <span className="inline-flex items-center gap-2">
      <button
        onClick={download}
        disabled={state === "busy"}
        className="inline-flex min-h-[44px] items-center rounded-full bg-coral px-6 font-bold text-white active:scale-[0.97] disabled:opacity-50"
      >
        {state === "busy" ? dict.torrent.downloading : dict.torrent.download}
      </button>
      {charged && (
        <span
          title={dict.torrent.priceHint ?? "首次下载将支付，重复下载不再扣费"}
          className="inline-flex min-h-[28px] items-center rounded-full bg-[var(--baozi-orange)] px-3 text-xs font-bold text-white"
        >
          {price} {currency}
        </span>
      )}
      {(price ?? 0) > 0 && purchased && !isOwner && (
        <span className="text-xs text-sub">{dict.torrent.purchased ?? "已购"}</span>
      )}
    </span>
  );
}
