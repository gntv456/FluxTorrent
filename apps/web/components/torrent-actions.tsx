"use client";

import { useEffect, useRef, useState } from "react";
import { useRouter } from "next/navigation";
import { api, ApiError } from "@/lib/api-client";
import { useI18n } from "@/i18n/client";

/** 种子行操作列（好学站口径）：下载钮 + ⋮ 下拉菜单（收藏 / 编辑 / 删除）。
 *  编辑/删除对无权者后端会拒（owner 或 staff），前端不预判权限、按需提示错误。
 *  菜单用 fixed 定位（0145 修复）：.nexus-table 有 overflow:hidden（圆角裁剪）、
 *  滚动包裹层 overflow-x:auto 连带把 y 也变 auto——种子少时最后一行的下拉菜单
 *  伸出表格底边即被裁。fixed 按按钮实测坐标定位可逃离全部祖先裁剪；
 *  下方空间不足自动向上翻；打开期间页面/容器滚动即关闭（避免菜单悬空错位）。 */
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
  const [menuPos, setMenuPos] = useState<{ top: number; right: number } | null>(
    null,
  );
  const [busy, setBusy] = useState(false);
  const [msg, setMsg] = useState<string | null>(null);
  const ref = useRef<HTMLSpanElement>(null);
  const moreBtnRef = useRef<HTMLButtonElement>(null);

  /** 打开菜单：按 ⋮ 钮的视口坐标计算 fixed 定位；下沿空间不足（估高 128px）改向上翻 */
  function openMenu() {
    const r = moreBtnRef.current?.getBoundingClientRect();
    if (!r) {
      setOpen(true);
      return;
    }
    const EST_H = 128; // 3 项 ×36px + 内边距，量级估算即可（翻向判断容差）
    const top =
      window.innerHeight - r.bottom < EST_H + 12
        ? Math.max(8, r.top - EST_H - 6)
        : r.bottom + 6;
    setMenuPos({ top, right: Math.max(8, window.innerWidth - r.right) });
    setOpen(true);
  }

  useEffect(() => {
    if (!open) return;
    const onDoc = (e: MouseEvent) => {
      if (ref.current && !ref.current.contains(e.target as Node))
        setOpen(false);
    };
    const onKey = (e: KeyboardEvent) => {
      if (e.key === "Escape") setOpen(false);
    };
    // capture：滚动可能发生在 overflow 容器内（表格横向滚动层）而非 window
    const onScroll = () => setOpen(false);
    document.addEventListener("mousedown", onDoc);
    document.addEventListener("keydown", onKey);
    window.addEventListener("scroll", onScroll, true);
    return () => {
      document.removeEventListener("mousedown", onDoc);
      document.removeEventListener("keydown", onKey);
      window.removeEventListener("scroll", onScroll, true);
    };
  }, [open]);

  async function bookmark() {
    setBusy(true);
    try {
      await api.put(`/api/v1/torrents/${torrentId}/bookmark`, { on: true });
      setMsg(dict.torrents.bookmarked);
      setOpen(false);
    } catch (e) {
      setMsg(e instanceof ApiError ? e.message : dict.common.networkError);
    } finally {
      setBusy(false);
    }
  }

  async function remove() {
    if (!window.confirm(dict.torrents.deleteConfirm))
      return;
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

  /** 列表行下载：与详情页同口径走 getBlob（带 Bearer）；裸 <a href> 无鉴权头必 401 */
  async function downloadRow() {
    setBusy(true);
    try {
      const data = await api.getBlob(`/api/v1/torrents/${torrentId}/download`);
      const blob = new Blob([data], { type: "application/x-bittorrent" });
      const url = URL.createObjectURL(blob);
      const a = document.createElement("a");
      a.href = url;
      a.download = `${torrentId}.torrent`;
      a.click();
      URL.revokeObjectURL(url);
    } catch (e) {
      const noauth = e instanceof ApiError && e.code === 2001;
      alert(
        noauth
          ? (dict.torrent.downloadNoauth)
          : e instanceof Error
            ? e.message
            : (dict.common.networkError),
      );
    } finally {
      setBusy(false);
    }
  }

  return (
    <span className="torrents-actions" ref={ref}>
      <button
        type="button"
        onClick={downloadRow}
        disabled={busy}
        className="torrents-action"
        title={downloadLabel}
        aria-label={downloadLabel}
      >
        ⬇
      </button>
      <button
        type="button"
        className="torrents-action"
        aria-haspopup="menu"
        aria-expanded={open}
        aria-label={dict.torrents.moreActions}
        title={dict.torrents.moreActions}
        ref={moreBtnRef}
        onClick={() => (open ? setOpen(false) : openMenu())}
      >
        ⋮
      </button>
      {open && menuPos && (
        <span
          className="torrents-menu torrents-menu--fixed"
          role="menu"
          style={{ top: menuPos.top, right: menuPos.right }}
        >
          <button
            type="button"
            role="menuitem"
            disabled={busy}
            onClick={bookmark}
          >
            ☆ {dict.torrents.bookmark}
          </button>
          <button type="button" role="menuitem" onClick={edit}>
            ✎ {dict.torrents.edit}
          </button>
          <button
            type="button"
            role="menuitem"
            className="danger"
            disabled={busy}
            onClick={remove}
          >
            🗑 {dict.torrents.delete}
          </button>
        </span>
      )}
      {msg && <span className="torrents-actions-msg">{msg}</span>}
    </span>
  );
}
