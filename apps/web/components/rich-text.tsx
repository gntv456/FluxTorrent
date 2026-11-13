"use client";

import { useEffect, useId, useRef, useState } from "react";

import { Icon } from "@/components/icons";
import { useI18n } from "@/i18n/client";

/** 富文本正文容器（种子简介 / 作品简介共用）。
 *  超长正文先给一屏多一点（限高走 CSS 的 .rt__body），底部渐隐 +
 *  「展开全部」按钮，展开后整段摊开——比原来 320px 的内滚条好用：
 *  内滚套内滚要两只手，展开是一眼到底。 */
export function RichText({ children }: { children: React.ReactNode }) {
  const { dict } = useI18n();
  const bodyRef = useRef<HTMLDivElement>(null);
  const innerRef = useRef<HTMLDivElement>(null);
  const [open, setOpen] = useState(false);
  const [over, setOver] = useState(false);
  const id = useId();

  useEffect(() => {
    // 展开态 max-height:none ⇒ 恒不溢出，此时再量会把「展开」误判成
    // 「本来就不长」，按钮会自己消失、收不回去。
    if (open) return;
    const body = bodyRef.current;
    const inner = innerRef.current;
    if (!body || !inner) return;
    const measure = () => {
      setOver(inner.scrollHeight > body.clientHeight + 8);
    };
    measure();
    // 图片迟到 / 尺寸变化会改内容高度 ⇒ 跟着重测
    const ro = new ResizeObserver(measure);
    ro.observe(inner);
    return () => ro.disconnect();
  }, [open]);

  return (
    <div className={open ? "rt is-open" : "rt"} data-rt-gallery="">
      <div className="rt__body" id={id} ref={bodyRef}>
        <div className="rt__inner" ref={innerRef}>
          {children}
        </div>
      </div>
      {over && (
        <div className="rt__more">
          {!open && <span className="rt__fade" aria-hidden="true" />}
          <button
            type="button"
            className="rt__toggle"
            aria-expanded={open}
            aria-controls={id}
            onClick={() => setOpen(!open)}
          >
            {open ? dict.ui.collapseAll : dict.ui.expandAll}
            <Icon
              name="chevronDown"
              size={16}
              className={open ? "rt__chev is-up" : "rt__chev"}
            />
          </button>
        </div>
      )}
    </div>
  );
}
