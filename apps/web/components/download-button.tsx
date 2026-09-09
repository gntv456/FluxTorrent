"use client";

import { useState } from "react";
import { api } from "@/lib/api-client";

/** 下载按钮：携带 Bearer token 以 blob 触发保存（后端 require_auth） */
export function DownloadButton({ torrentId, name }: { torrentId: number; name: string }) {
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
      setState(e instanceof Error && e.message.includes("登录") ? "noauth" : "idle");
      if (!(e instanceof Error) || !e.message.includes("登录")) {
        alert(e instanceof Error ? e.message : "下载失败，请先登录");
      } else {
        alert("请先登录后再下载");
      }
    }
  }

  return (
    <button
      onClick={download}
      disabled={state === "busy"}
      className="mt-4 inline-flex min-h-[44px] items-center rounded-full bg-coral px-6 font-bold text-white active:scale-[0.97] disabled:opacity-50"
    >
      {state === "busy" ? "下载中…" : "下载种子"}
    </button>
  );
}
