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
      {/* P0：关闭时保留 DOM（只做视觉隐藏），不能卸载子树——
          菜单里的「举报主题」「编辑标题」各自用本地 open state 控制 Modal，
          一旦卸载，子 state 丢失，弹窗永远打不开。
          display:none 保留宿主节点，onMouseDown 的「点外部关闭」仍走根容器。 */}
      <div
        className="thmenu__drop tp-headmenu__drop"
        role="menu"
        style={{ display: open ? undefined : "none" }}
      >
        {items.map((it, i) => (
          <div key={i} role="menuitem" className="tp-headmenu__item">
            {it}
          </div>
        ))}
      </div>
    </div>
  );
}
