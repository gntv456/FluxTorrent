"use client";

import { useEffect, useRef, useState, type ReactNode } from "react";
import { Icon } from "@/components/icons";
import { useFocusTrap } from "@/lib/use-focus-trap";
import { useI18n } from "@/i18n/client";

/** <md：搜索整块（常驻行 + 已选摘要 + 高级面板）收进底部弹层，
 *  由右下角「搜索」唤起；≥md 原样内联。只看一份表单 DOM，
 *  避免同名 name/id 渲染两遍（联想下拉会绑错输入框）。 */
export function MobileSearch({ children }: { children: ReactNode }) {
  const { dict } = useI18n();
  const [open, setOpen] = useState(false);
  const panelRef = useRef<HTMLDivElement>(null);
  useFocusTrap(open, panelRef);

  useEffect(() => {
    if (!open) return;
    const prev = document.body.style.overflow;
    document.body.style.overflow = "hidden";
    const onKey = (e: KeyboardEvent) => {
      if (e.key === "Escape") setOpen(false);
    };
    document.addEventListener("keydown", onKey);
    return () => {
      document.body.style.overflow = prev;
      document.removeEventListener("keydown", onKey);
    };
  }, [open]);

  return (
    <>
      <div
        ref={panelRef}
        className={`tsb-sheet${open ? " is-open" : ""}`}
        role={open ? "dialog" : undefined}
        aria-modal={open || undefined}
        aria-label={open ? dict.torrents2.searchTitle : undefined}
      >
        <div className="tsb-sheet__hd md:hidden">
          <p className="tsb-sheet__title">{dict.torrents2.searchTitle}</p>
          <button
            type="button"
            className="tsb-sheet__close"
            aria-label={dict.common.cancel}
            onClick={() => setOpen(false)}
          >
            ×
          </button>
        </div>
        <div className="tsb-sheet__body">{children}</div>
      </div>
      {open && (
        <div
          className="tsb-sheet__mask"
          aria-hidden="true"
          onClick={() => setOpen(false)}
        />
      )}
      <button
        type="button"
        className="search-fab md:hidden"
        aria-label={dict.torrents2.searchTitle}
        onClick={() => setOpen(true)}
      >
        <Icon name="search" size={18} />
        {dict.torrents2.searchTitle}
      </button>
    </>
  );
}
