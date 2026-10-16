"use client";

import { useEffect, useRef, useState } from "react";
import { createPortal } from "react-dom";

import { useFocusTrap } from "@/lib/use-focus-trap";

/** 覆盖式弹窗（0173）：fixed 遮罩 + 居中卡片，ESC / 点遮罩关闭，锁 body 滚动。
 *  详情页编辑（TorrentManage）、购买置顶免费（PromoBuyButton）共用。
 *
 *  必须 portal 到 body（2026-10-16 实测）：从「更多操作」面板里再打开时，
 *  卡片是 `.more-sheet` 的后代，而该面板开着时带 `transform` 且
 *  `max-height:72vh; overflow-y:auto` —— `position:fixed` 会改成相对该面板
 *  定位并被裁切（实测 1440×900 视口下遮罩只有 520×338），层级也压回面板下。 */
const WIDTH = { md: "max-w-2xl", lg: "max-w-4xl" } as const;

export function Modal({
  open,
  onClose,
  title,
  size = "md",
  children,
}: {
  open: boolean;
  onClose: () => void;
  title: string;
  /** 卡片宽度档位：md=672px（默认，轻量确认/单选），lg=896px（多字段表单） */
  size?: keyof typeof WIDTH;
  children: React.ReactNode;
}) {
  const cardRef = useRef<HTMLDivElement>(null);
  // SSR 期没有 document，渲染期直接 createPortal 会抛 ReferenceError
  // （同 torrent-head-more 的 mounted 闸，不用 typeof document 判——
  //  那会让服务端与客户端首帧输出不一致）。
  const [mounted, setMounted] = useState(false);
  useEffect(() => setMounted(true), []);
  // 焦点圈定：Tab 不穿透到遮罩后的页面（ZT81）
  useFocusTrap(open, cardRef);

  useEffect(() => {
    if (!open) return;
    const onKey = (e: KeyboardEvent) => {
      if (e.key === "Escape") onClose();
    };
    document.addEventListener("keydown", onKey);
    const prev = document.body.style.overflow;
    document.body.style.overflow = "hidden";
    return () => {
      document.removeEventListener("keydown", onKey);
      document.body.style.overflow = prev;
    };
  }, [open, onClose]);

  if (!open || !mounted) return null;
  return createPortal(
    <div
      className="fixed inset-0 z-[var(--z-modal-nested)] flex items-start justify-center overflow-y-auto bg-black/55 p-4 sm:p-8"
      onClick={onClose}
      role="presentation"
    >
      <div
        ref={cardRef}
        role="dialog"
        aria-modal
        aria-label={title}
        className={`mt-6 w-full ${WIDTH[size]} rounded-[var(--r-md)] border border-line bg-[var(--surface-card)] p-4 shadow-[var(--shadow-card)] sm:mt-12`}
        onClick={(e) => e.stopPropagation()}
      >
        <div className="mb-3 flex items-center justify-between">
          <h3 className="text-sm font-bold">{title}</h3>
          <button
            type="button"
            onClick={onClose}
            aria-label="close"
            className="flex h-7 w-7 items-center justify-center rounded-full border border-line text-xs text-sub"
          >
            ×
          </button>
        </div>
        {children}
      </div>
    </div>,
    document.body,
  );
}
