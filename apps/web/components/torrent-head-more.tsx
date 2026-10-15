"use client";

import { useEffect, useState } from "react";
import { createPortal } from "react-dom";
import { useIsCompact } from "@/lib/hooks/use-media";
import { REPLY_EVENT } from "@/components/comment-reply-button";

/** 头部「更多操作」收纳（M5.10/M5.11；M6.5 改全断点面板）：
 *  桌面与移动端都是「trigger + 面板」——桌面 trigger 内联渲染在头部行 2
 *  （ghost 钮，本地完善版样图头部只留 下载/心愿单/感谢/更多 四钮），
 *  移动端 trigger 经 portal 进底部吸底操作条作第四钮；面板两端共用
 *  （桌面居中窄条、移动端贴底，样式见 base.css .more-sheet 两档）。
 *  面板恒挂载（inert 隐藏式）：评论「回复」按钮依赖 REPLY_EVENT
 *  聚焦 #td-comment-input，children 必须常在 DOM；收到回复事件时
 *  自动弹出面板让输入框可见（全断点——评论框已不在头部常驻）。 */
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
  // （仅移动端需要：桌面 trigger 内联渲染，不进吸底条）
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

  // 评论「回复」→ 自动弹出面板（全断点：评论框在面板内）并补一次聚焦——
  // 回复钮自身的 focus() 会撞上面板尚未摘 inert 的时刻，故延后一拍重试。
  useEffect(() => {
    const onReply = () => {
      setOpen(true);
      setTimeout(() => {
        document.getElementById("td-comment-input")?.focus();
      }, 80);
    };
    window.addEventListener(REPLY_EVENT, onReply);
    return () => window.removeEventListener(REPLY_EVENT, onReply);
  }, []);

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

  const trigger = (
    <button
      type="button"
      className={`js-td-more ${compact ? "td-actionbar__btn" : "td-more-btn"}`}
      aria-haspopup="dialog"
      aria-expanded={open}
      onClick={() => setOpen(true)}
    >
      {label}
    </button>
  );

  const sheet = createPortal(
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
  );

  // 移动端头部操作区整块 display:none，trigger 只能挂吸底条
  if (compact) {
    return (
      <>
        {bar && createPortal(trigger, bar)}
        {sheet}
      </>
    );
  }
  return (
    <>
      {trigger}
      {sheet}
    </>
  );
}
