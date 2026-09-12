"use client";

import { useEffect, useRef, useState } from "react";
import { useRouter } from "next/navigation";
import { api, ApiError } from "@/lib/api-client";
import { useI18n } from "@/i18n/client";

/** 种子行操作列（好学站口径）：下载钮 + ⋮ 下拉菜单（收藏 / 编辑 / 删除）。
 *  编辑/删除对无权者后端会拒（owner 或 staff），前端不预判权限、按需提示错误。 */
export function TorrentActions({
  torrentId,
  downloadLabel,
}: {
  torrentId: number;
  downloadLabel: string;
}) {
  const { dict } = useI18n();
  const router = useRouter();
  const [open, setOpen] = useState(false);
  const [busy, setBusy] = useState(false);
  const [msg, setMsg] = useState<string | null>(null);
  const ref = useRef<HTMLSpanElement>(null);

  useEffect(() => {
    if (!open) return;
    const onDoc = (e: MouseEvent) => {
      if (ref.current && !ref.current.contains(e.target as Node)) setOpen(false);
    };
    const onKey = (e: KeyboardEvent) => {
      if (e.key === "Escape") setOpen(false);
    };
    document.addEventListener("mousedown", onDoc);
    document.addEventListener("keydown", onKey);
    return () => {
      document.removeEventListener("mousedown", onDoc);
      document.removeEventListener("keydown", onKey);
    };
  }, [open]);

  async function bookmark() {
    setBusy(true);
    try {
      await api.put(`/api/v1/torrents/${torrentId}/bookmark`, { on: true });
      setMsg(dict.torrents.bookmarked ?? "已收藏");
      setOpen(false);
    } catch (e) {
      setMsg(e instanceof ApiError ? e.message : dict.common.networkError);
    } finally {
      setBusy(false);
    }
  }

  async function remove() {
    if (!window.confirm(dict.torrents.deleteConfirm ?? "确认删除该种子？")) return;
    setBusy(true);
    try {
      await api.del(`/api/v1/torrents/${torrentId}`);
      setOpen(false);
      router.refresh();
    } catch (e) {
      setMsg(e instanceof ApiError ? e.message : dict.common.networkError);
    } finally {
      setBusy(false);
    }
  }

  function edit() {
    setOpen(false);
    router.push(`/torrent/${torrentId}?edit=1`);
  }

  return (
    <span className="torrents-actions" ref={ref}>
      <a
        href={`/api/v1/torrents/${torrentId}/download`}
        className="torrents-action"
        title={downloadLabel}
        aria-label={downloadLabel}
      >
        ⬇
      </a>
      <button
        type="button"
        className="torrents-action"
        aria-haspopup="menu"
        aria-expanded={open}
        aria-label={dict.torrents.moreActions ?? "更多操作"}
        title={dict.torrents.moreActions ?? "更多操作"}
        onClick={() => setOpen((v) => !v)}
      >
        ⋮
      </button>
      {open && (
        <span className="torrents-menu" role="menu">
          <button type="button" role="menuitem" disabled={busy} onClick={bookmark}>
            ☆ {dict.torrents.bookmark ?? "收藏"}
          </button>
          <button type="button" role="menuitem" onClick={edit}>
            ✎ {dict.torrents.edit ?? "编辑"}
          </button>
          <button type="button" role="menuitem" className="danger" disabled={busy} onClick={remove}>
            🗑 {dict.torrents.delete ?? "删除"}
          </button>
        </span>
      )}
      {msg && <span className="torrents-actions-msg">{msg}</span>}
    </span>
  );
}
