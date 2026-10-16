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
/** 卡片宽度按「形态 + 档位」定：抽屉不跟居中档走（max-w-2xl 会吃掉整屏） */
const CARD = {
  "center-sm": "mt-6 w-full max-w-sm",
  "center-md": "mt-6 w-full max-w-2xl",
  "center-lg": "mt-6 w-full max-w-4xl",
  "right-sm": "h-full w-full max-w-[280px]",
  "right-md": "h-full w-full max-w-md",
  "right-lg": "h-full w-full max-w-xl",
} as const;
const OVERLAY = {
  center: "items-start justify-center p-4 sm:p-8",
  right: "items-stretch justify-end",
} as const;

export function Modal({
  open,
  onClose,
  title,
  size = "md",
  align = "center",
  children,
}: {
  open: boolean;
  onClose: () => void;
  title: string;
  /** 卡片宽度档位：md=672px（默认，轻量确认/单选），lg=896px（多字段表单） */
  size?: "sm" | "md" | "lg";
  /** center=居中小卡（默认）；right=右侧全高抽屉。抽屉此前各页手写
   *  `fixed inset-0 z-30/z-50`：z 低于吸顶头栏（50）、又没 portal，于是弹窗
   *  上沿被头栏压住、Tab 还会穿透到页面（2026-10-16 弹窗普查 A3/A4）。
   *  形态保留，层叠与键盘语义统一走这个标准件。 */
  align?: "center" | "right";
  children: React.ReactNode;
}) {
  const cardRef = useRef<HTMLDivElement>(null);
  // SSR 期没有 document，渲染期直接 createPortal 会抛 ReferenceError
  // （同 torrent-head-more 的 mounted 闸，不用 typeof document 判——
  //  那会让服务端与客户端首帧输出不一致）。
  const [mounted, setMounted] = useState(false);
  useEffect(() => setMounted(true), []);
  // 焦点圈定：Tab 不穿透到遮罩后的页面（ZT81）。
  // 必须带上 mounted：调用方常见写法是 `{cond && <Modal open …/>}`（挂载即打开），
  // 那一帧 mounted 还是 false ⇒ 卡片没渲染、cardRef.current 为 null，陷阱的依赖 [open, ref]
  // 之后不再变化 ⇒ 陷阱永远挂不上（2026-10-16 抽屉收口时实测：Tab 十二次全跑到背景）。
  useFocusTrap(open && mounted, cardRef);

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

  // 类串提出来拼：模板串整行塞进 JSX 会顶破 80 列行宽门禁
  const scrim =
    "fixed inset-0 z-[var(--z-modal-nested)] flex overflow-y-auto " +
    `bg-black/55 ${OVERLAY[align]}`;
  const card =
    `${CARD[`${align}-${size}` as keyof typeof CARD]} ` +
    "border border-line bg-[var(--surface-card)] p-4 " +
    "shadow-[var(--shadow-card)] " +
    (align === "center"
      ? "rounded-[var(--r-md)] sm:mt-12"
      : "border-y-0 border-r-0");

  if (!open || !mounted) return null;
  return createPortal(
    <div
      className={scrim}
      onClick={onClose}
      role="presentation"
    >
      <div
        ref={cardRef}
        role="dialog"
        aria-modal
        aria-label={title}
        className={card}
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
