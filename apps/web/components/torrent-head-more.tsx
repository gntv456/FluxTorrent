"use client";

import { useEffect, useState } from "react";
import { createPortal } from "react-dom";
import { useIsCompact } from "@/lib/hooks/use-media";
import { REPLY_EVENT } from "@/components/comment-reply-button";

/** 头部「更多操作」收纳（M5.10/M5.11）：桌面原样渲染 children（零
 *  变化）；移动端头部不留任何操作区——触发钮经 portal 渲染进底部
 *  吸底操作条（#td-actionbar）作第四钮，点击弹出底部操作面板。
 *  面板恒挂载（inert 隐藏式）：评论「回复」按钮依赖 REPLY_EVENT
 *  聚焦 #td-comment-input，children 必须常在 DOM；收到回复事件时
 *  自动弹出面板让输入框可见。 */
export function HeadActionsMore({
  label,
  children,
}: {
  /** 触发钮与面板标题文案（dict.torrents.moreActions） */
  label: string;
  children: React.ReactNode;
}) {
  const compact = useIsCompact();
  const [open, setOpen] = useState(false);
  const [bar, setBar] = useState<HTMLElement | null>(null);

  // 吸底栏与本组件按各自断点翻转，挂载时序不定——轮询等容器就绪
  useEffect(() => {
    if (!compact) return;
    let tries = 0;
    let timer: ReturnType<typeof setTimeout> | undefined;
    const hunt = () => {
      const el = document.getElementById("td-actionbar");
      if (el) {
        setBar(el);
      } else if (tries++ < 20) {
        timer = setTimeout(hunt, 100);
      }
    };
    hunt();
    return () => clearTimeout(timer);
  }, [compact]);

  // 评论「回复」→ 自动弹出面板（评论输入框在其中，聚焦才能命中）
  useEffect(() => {
    if (!compact) return;
    const onReply = () => setOpen(true);
    window.addEventListener(REPLY_EVENT, onReply);
    return () => window.removeEventListener(REPLY_EVENT, onReply);
  }, [compact]);

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
      {bar &&
        createPortal(
          <button
            type="button"
            className="td-actionbar__btn js-td-more"
            aria-haspopup="dialog"
            aria-expanded={open}
            onClick={() => setOpen(true)}
          >
            {label}
          </button>,
          bar,
        )}
      {/* 面板 portal 到 body：本组件宿主在 .td-head__actions 内，
          该容器 <768 被 display:none——不 portal 面板会跟着陪葬 */}
      {createPortal(
        <div
          className={`more-sheet-mask${open ? " is-open" : " is-closed"}`}
          onClick={() => setOpen(false)}
          aria-hidden={!open}
          inert={!open}
        >
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
        </div>,
        document.body,
      )}
    </>
  );
}
