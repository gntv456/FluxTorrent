"use client";

import { useState } from "react";
import { api, ApiError } from "@/lib/api-client";
import { useI18n } from "@/i18n/client";
import { toast } from "@/components/ui/toast";

/** 磁力按钮（六维 2.4）：GET /torrents/{id}/magnet → 复制到剪贴板，
 *  announce 与 .torrent 同源（站点设定优先），无需再下文件。
 *  M6.5：桌面头部按样图只留下载+心愿单，磁力钮收进「更多操作」面板，
 *  故拆为独立组件供两处复用（移动端吸底条的复制钮另走 actionbar 壳）。 */
export function MagnetButton({ torrentId }: { torrentId: number }) {
  const { dict } = useI18n();
  const [magnetState, setMagnetState] = useState<
    "idle" | "busy" | "copied"
  >("idle");

  async function copyMagnet() {
    setMagnetState("busy");
    try {
      const r = await api.get<{ magnet: string }>(
        `/api/v1/torrents/${torrentId}/magnet`,
      );
      await navigator.clipboard.writeText(r.magnet);
      setMagnetState("copied");
      setTimeout(() => setMagnetState("idle"), 1500);
    } catch (e) {
      setMagnetState("idle");
      // W-24：失败 toast 带 request_id 芯片（可复制报障）
      toast.error(dict.torrent.magnetFailed, {
        requestId: e instanceof ApiError ? e.requestId : undefined,
      });
    }
  }

  return (
    <button
      onClick={copyMagnet}
      disabled={magnetState === "busy"}
      title={dict.torrent.magnetCopied}
      className="td-dl-btn td-dl-btn--magnet"
    >
      {magnetState === "copied"
        ? dict.torrent.magnetCopied
        : dict.torrent.magnet}
    </button>
  );
}

/** 下载按钮：携带 Bearer token 以 blob 触发保存（后端 require_auth）
 *  付费种子（0086）：显示价格角标；扣费/已购由后端 download 原子处理，余额不足会弹错。
 *  showMagnet=false 时只出下载钮（磁力钮由调用方放进「更多操作」面板）。 */
export function DownloadButton({
  torrentId,
  name,
  price,
  purchased,
  isOwner,
  showMagnet = true,
}: {
  torrentId: number;
  name: string;
  price?: number;
  purchased?: boolean;
  isOwner?: boolean;
  showMagnet?: boolean;
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
      // W-24 反馈协议：alert → toast.error（request_id 可诊断）
      toast.error(
        noauth
          ? dict.torrent.downloadNoauth
          : e instanceof Error
            ? e.message
            : dict.torrent.downloadFailed,
        { requestId: e instanceof ApiError ? e.requestId : undefined },
      );
    }
  }

  return (
    <span className="inline-flex items-center gap-2">
      <button
        onClick={download}
        disabled={state === "busy"}
        className="td-dl-btn"
      >
        {state === "busy" ? dict.torrent.downloading : dict.torrent.download}
      </button>
      {showMagnet && <MagnetButton torrentId={torrentId} />}
      {charged && (
        <span
          title={dict.torrent.priceHint}
          className="td-dl-price"
        >
          {price} {currency}
        </span>
      )}
      {(price ?? 0) > 0 && purchased && !isOwner && (
        <span className="text-xs text-sub">
          {dict.torrent.purchased}
        </span>
      )}
    </span>
  );
}
