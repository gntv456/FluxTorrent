"use client";

import { useState } from "react";
import { api, ApiError } from "@/lib/api-client";
import { useI18n } from "@/i18n/client";

/** 下载按钮：携带 Bearer token 以 blob 触发保存（后端 require_auth） */
export function DownloadButton({ torrentId, name }: { torrentId: number; name: string }) {
  const { dict } = useI18n();
  const [state, setState] = useState<"idle" | "busy" | "noauth">("idle");

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
    <button
      onClick={download}
      disabled={state === "busy"}
      className="mt-4 inline-flex min-h-[44px] items-center rounded-full bg-coral px-6 font-bold text-white active:scale-[0.97] disabled:opacity-50"
    >
      {state === "busy" ? dict.torrent.downloading : dict.torrent.download}
    </button>
  );
}
