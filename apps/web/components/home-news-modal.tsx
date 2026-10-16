"use client";

import { useEffect, useRef } from "react";

import { useI18n } from "@/i18n/client";

/** 首页公告正文弹窗。
 *
 *  原先是 `<dialog className="home-news-modal" open>`——只写 `open` 是「常显」
 *  而不是「模态」：浏览器不给它 top-layer、不给 Esc 关闭、不圈焦点，读屏也
 *  拿不到模态语义。改成挂载时 `showModal()`、并监听原生 `close` 事件回写
 *  父级状态（否则 Esc 关掉后 `modal` 还留着，再点同一条不会重开）。
 *  样式类名沿用 layout.css 的 `.home-news-modal*`，外观不变。 */
export function HomeNewsModal({
  item,
  onClose,
}: {
  item: { title: string; body: string };
  onClose: () => void;
}) {
  const t = useI18n().dict.home2;
  const ref = useRef<HTMLDialogElement>(null);

  useEffect(() => {
    const el = ref.current;
    if (!el) return;
    el.showModal();
    // Esc / 点遮罩都由原生 close 事件收敛，统一回写父级，避免状态滞留
    el.addEventListener("close", onClose);
    return () => el.removeEventListener("close", onClose);
  }, [onClose]);

  return (
    <dialog
      ref={ref}
      className="home-news-modal"
      onClick={(e) => e.target === e.currentTarget && onClose()}
    >
      <section className="home-news-modal__panel">
        <header className="home-news-modal__head">
          <div>
            <span>{t.newsBadge}</span>
            <h2>{item.title}</h2>
          </div>
          <button
            type="button"
            aria-label={t.closeModal}
            title={t.closeModal}
            onClick={onClose}
          >
            ×
          </button>
        </header>
        {/* 视频内嵌（0190）：iframe/video 全宽 16:9 响应式（任意子代选择器） */}
        <div
          className={"home-news-modal__body " +
            "[&_iframe]:block [&_iframe]:aspect-video " +
            "[&_iframe]:w-full [&_iframe]:max-w-full " +
            "[&_iframe]:border-0 [&_video]:w-full [&_video]:max-w-full"}
          dangerouslySetInnerHTML={{ __html: item.body }}
        />
      </section>
    </dialog>
  );
}
