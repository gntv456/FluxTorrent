"use client";

import { useEffect, useState } from "react";
import { useIsCompact } from "@/lib/hooks/use-media";

/** 头部「更多操作」收纳（M5.10）：桌面原样渲染 children（零变化），
 *  移动端只渲染一个「更多操作」触发按钮，点击弹出底部操作面板——
 *  感谢/收藏/评论/打赏/抢种/复活/管理等次要操作不再平铺占屏，
 *  下载主按钮保留在头部常驻。底部 TabBar 常驻后，头部按钮平铺
 *  属重复占地。与 FoldMobile 同范式：SSR 首帧桌面态，水合后切换。 */
export function HeadActionsMore({
  label,
  children,
}: {
  /** 触发按钮与面板标题文案（dict.torrents.moreActions） */
  label: string;
  children: React.ReactNode;
}) {
  const compact = useIsCompact();
  const [open, setOpen] = useState(false);
  // 打开时锁页面滚动 + Esc 关闭
  useEffect(() => {
    if (!open) return;
    const onKey = (e: KeyboardEvent) => {
      if (e.key === "Escape") setOpen(false);
    };
    document.body.style.overflow = "hidden";
    window.addEventListener("keydown", onKey);
    return () => {
      document.body.style.overflow = "";
      window.removeEventListener("keydown", onKey);
    };
  }, [open]);
  if (!compact) return <>{children}</>;
  return (
    <>
      <button
        type="button"
        className="more-sheet__trigger"
        aria-haspopup="dialog"
        aria-expanded={open}
        onClick={() => setOpen(true)}
      >
        {label} <span aria-hidden>⋯</span>
      </button>
      {open && (
        <div className="more-sheet-mask" onClick={() => setOpen(false)}>
          <div
            className="more-sheet"
            role="dialog"
            aria-modal="true"
            aria-label={label}
            onClick={(e) => e.stopPropagation()}
          >
            <p className="more-sheet__title">{label}</p>
            <button
              type="button"
              className="more-sheet__close"
              aria-label={label}
              onClick={() => setOpen(false)}
            >
              ×
            </button>
            {children}
          </div>
        </div>
      )}
    </>
  );
}
