"use client";

import { useCallback, useEffect, useRef, useState } from "react";
import { api, ApiError } from "@/lib/api-client";
import { useI18n } from "@/i18n/client";
import { formatBytes } from "@/lib/format";
import { Fold } from "@/components/torrent-detail-parts";

interface PreviewItem {
  sha256: string;
  filename: string;
  mime: string;
  size: number;
  /** preview（图文样章）/ audio（试听片段） */
  kind: string;
}

/** 试读 / 试听（0337）：清单 + 管理入口。
 *
 *  管理按钮**常显、权限由后端 403 兜底**（与删评按钮同一范式）：
 *  作者/staff 走「先传附件拿 sha256 → 再关联」两步；移除只解除关联，
 *  物理文件仍归附件体系。凭证走 HttpOnly cookie（与图床上传同款）。
 *  `audio` 用 <audio controls> 内联播放，其余（图文样章）给新窗链接。 */
export function TorrentPreviews({ torrentId }: { torrentId: number }) {
  const { dict } = useI18n();
  const d = dict.tdetail;
  const [items, setItems] = useState<PreviewItem[]>([]);
  const [busy, setBusy] = useState(false);
  const [msg, setMsg] = useState<string | null>(null);
  const fileRef = useRef<HTMLInputElement>(null);

  const load = useCallback(() => {
    api
      .get<{ items: PreviewItem[] }>(
        `/api/v1/torrents/${torrentId}/previews`,
      )
      .then((r) => setItems(r.items ?? []))
      .catch(() => setItems([]));
  }, [torrentId]);

  useEffect(() => {
    load();
  }, [load]);

  async function add(file: File) {
    setBusy(true);
    setMsg(null);
    try {
      const fd = new FormData();
      fd.append("file", file);
      const res = await fetch("/api/v1/attachments", {
        method: "POST",
        body: fd,
      });
      const j = (await res.json()) as {
        data?: { sha256?: string };
        message?: string;
      };
      const sha = j.data?.sha256;
      if (!res.ok || !sha) throw new Error(j.message ?? d.reportFail);
      await api.post(`/api/v1/torrents/${torrentId}/previews`, {
        sha256: sha,
        filename: file.name,
      });
      setMsg(d.previewDone);
      load();
    } catch (e) {
      setMsg(e instanceof Error ? e.message : d.reportFail);
    } finally {
      setBusy(false);
    }
  }

  async function remove(sha: string) {
    setBusy(true);
    setMsg(null);
    try {
      const res = await fetch(`/api/v1/torrents/${torrentId}/previews`, {
        method: "DELETE",
        headers: { "Content-Type": "application/json" },
        body: JSON.stringify({ sha256: sha }),
      });
      if (!res.ok) {
        const j = (await res.json().catch(() => ({}))) as { message?: string };
        throw new Error(j.message ?? d.reportFail);
      }
      load();
    } catch (e) {
      setMsg(e instanceof Error ? e.message : d.reportFail);
    } finally {
      setBusy(false);
    }
  }

  return (
    <Fold title={d.previewTitle} count={items.length}>
      {items.length > 0 && (
        <ul className="td-previews">
          {items.map((p) => {
            const url = `/api/v1/attachments/${p.sha256}`;
            return (
              <li key={p.sha256} className="td-previews__item">
                {p.kind === "audio" ? (
                  <audio controls preload="none" src={url} />
                ) : (
                  <a href={url} target="_blank" rel="noreferrer">
                    {p.filename}
                  </a>
                )}
                <span className="text-xs text-sub">{formatBytes(p.size)}</span>
                <button
                  type="button"
                  className="sticker ml-1"
                  disabled={busy}
                  onClick={() => remove(p.sha256)}
                >
                  {d.previewRemove}
                </button>
              </li>
            );
          })}
        </ul>
      )}
      <div className="mt-2 flex items-center gap-2">
        <input
          ref={fileRef}
          type="file"
          className="sr-only"
          onChange={(e) => {
            const f = e.target.files?.[0];
            if (f) void add(f);
            e.target.value = "";
          }}
        />
        <button
          type="button"
          className="sticker"
          disabled={busy}
          onClick={() => fileRef.current?.click()}
        >
          {busy ? d.previewUploading : d.previewAdd}
        </button>
        {msg && <span className="text-xs text-sub">{msg}</span>}
      </div>
    </Fold>
  );
}
