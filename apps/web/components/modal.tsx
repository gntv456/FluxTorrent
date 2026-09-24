"use client";

import { useEffect } from "react";

/** 覆盖式弹窗（0173）：fixed 遮罩 + 居中卡片，ESC / 点遮罩关闭，锁 body 滚动。
 *  详情页编辑（TorrentManage）、购买置顶免费（PromoBuyButton）共用。 */
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

  if (!open) return null;
  return (
    <div
      className="fixed inset-0 z-50 flex items-start justify-center overflow-y-auto bg-black/55 p-4 sm:p-8"
      onClick={onClose}
      role="presentation"
    >
      <div
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
    </div>
  );
}
