"use client";

import { useEffect, useRef, useState } from "react";
import type { ReactNode } from "react";

/** 帖子页头部「更多」下拉（2026-11-12 阅读性改版）：
 *  关注/收藏/举报/编辑标题原平铺在标题行——长标题帖一排按钮把标题挤
 *  成两行、与正文抢注意力。收进一枚 ⋮ 触发器，菜单项由调用侧传
 *  （node 数组原样渲染，样式统一在菜单层）；外点/Esc 关闭
 *  （thmenu 同范式）。无菜单项（游客）时不渲染触发器。 */
export function TopicHeadMenu({ items }: { items: ReactNode[] }) {
  const [open, setOpen] = useState(false);
  const rootRef = useRef<HTMLDivElement>(null);

  useEffect(() => {
    if (!open) return;
    const onDoc = (e: MouseEvent) => {
      if (rootRef.current && !rootRef.current.contains(e.target as Node))
        setOpen(false);
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

  if (items.length === 0) return null;
  return (
    <div className="thmenu" ref={rootRef}>
      <button
        type="button"
        className="tp-headmenu__trigger"
        aria-haspopup="menu"
        aria-expanded={open}
        aria-label="⋯"
        onClick={() => setOpen((v) => !v)}
      >
        ⋯
      </button>
      {open ? (
        <div className="thmenu__drop tp-headmenu__drop" role="menu">
          {items.map((it, i) => (
            <div
              key={i}
              role="menuitem"
              className="tp-headmenu__item"
              onClick={() => setOpen(false)}
            >
              {it}
            </div>
          ))}
        </div>
      ) : null}
    </div>
  );
}
