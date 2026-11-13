"use client";

import { useEffect, useRef, useState } from "react";
import { createPortal } from "react-dom";

import { Icon } from "@/components/icons";
import { useI18n } from "@/i18n/client";
import { useFocusTrap } from "@/lib/use-focus-trap";

/** 图片放大浮层（简介图 / BBCode 图点击放大）。
 *  portal + --z-modal-nested，ESC / 点背景关闭，锁 body 滚动，焦点圈定。
 *  两档缩放：适应窗口看全图 ⇄ 原始尺寸看细节（超框内滚）。
 *  多图时 ← → / 两侧按钮切图，触屏横向滑动切图。 */
export function ImageLightbox({
  srcs,
  index,
  onIndex,
  onClose,
}: {
  srcs: string[];
  index: number;
  onIndex: (i: number) => void;
  onClose: () => void;
}) {
  const { dict } = useI18n();
  const rootRef = useRef<HTMLDivElement>(null);
  const imgRef = useRef<HTMLImageElement>(null);
  const touchX = useRef(0);
  const [fit, setFit] = useState(true);
  const [state, setState] = useState<"loading" | "ok" | "error">("loading");
  const src = srcs[index] ?? "";
  const many = srcs.length > 1;

  useFocusTrap(true, rootRef);

  useEffect(() => {
    // 缓存命中时 load 事件可能早于本 effect ⇒ 按 complete 兜底判定，
    // 否则「早就加载完」的图会永远停在 loading 态。
    const el = imgRef.current;
    if (!el || !el.complete) setState("loading");
    else setState(el.naturalWidth > 0 ? "ok" : "error");
    setFit(true);
  }, [src]);

  useEffect(() => {
    const step = (delta: number) => {
      if (!many) return;
      onIndex((index + delta + srcs.length) % srcs.length);
    };
    const onKey = (e: KeyboardEvent) => {
      if (e.key === "Escape") {
        onClose();
        return;
      }
      if (e.key === "ArrowLeft") step(-1);
      if (e.key === "ArrowRight") step(1);
    };
    document.addEventListener("keydown", onKey);
    const prev = document.body.style.overflow;
    document.body.style.overflow = "hidden";
    return () => {
      document.removeEventListener("keydown", onKey);
      document.body.style.overflow = prev;
    };
  }, [index, many, srcs.length, onClose, onIndex]);

  const stop = (e: React.MouseEvent) => e.stopPropagation();

  return createPortal(
    <div
      className="ilb"
      ref={rootRef}
      role="dialog"
      aria-modal
      aria-label={dict.ui.imageViewer}
      onClick={onClose}
    >
      <div className="ilb__bar" onClick={stop}>
        <span className="ilb__count num" aria-live="polite">
          {index + 1} / {srcs.length}
        </span>
        <div className="ilb__acts">
          <button
            type="button"
            className="ilb__btn"
            onClick={() => setFit(!fit)}
          >
            {fit ? dict.ui.zoomActual : dict.ui.zoomFit}
          </button>
          <a
            className="ilb__btn"
            href={src}
            target="_blank"
            rel="noreferrer noopener"
          >
            {dict.ui.viewOriginal}
          </a>
          <button
            type="button"
            className="ilb__btn ilb__btn--icon"
            onClick={onClose}
            data-autofocus
            aria-label={dict.ui.closeViewer}
          >
            <Icon name="close" size={18} />
          </button>
        </div>
      </div>
      <div
        className="ilb__stage"
        onTouchStart={(e) => {
          touchX.current = e.touches[0]?.clientX ?? 0;
        }}
        onTouchEnd={(e) => {
          const dx = (e.changedTouches[0]?.clientX ?? 0) - touchX.current;
          if (!many || Math.abs(dx) < 48) return;
          onIndex((index + (dx < 0 ? 1 : -1) + srcs.length) % srcs.length);
        }}
      >
        {state === "error" ? (
          <div className="ilb__err">
            <p>{dict.ui.imageFailed}</p>
            <a href={src} target="_blank" rel="noreferrer noopener">
              {dict.ui.viewOriginal}
            </a>
          </div>
        ) : (
          <>
            {state === "loading" && (
              <span className="ilb__hint">{dict.common.loading}</span>
            )}
            <img
              ref={imgRef}
              src={src}
              alt=""
              className={fit ? "ilb__img" : "ilb__img ilb__img--full"}
              onClick={(e) => {
                e.stopPropagation();
                setFit(!fit);
              }}
              onLoad={() => setState("ok")}
              onError={() => setState("error")}
            />
          </>
        )}
        {many && (
          <>
            <button
              type="button"
              className="ilb__nav ilb__nav--prev"
              aria-label={dict.ui.prevImage}
              onClick={(e) => {
                e.stopPropagation();
                onIndex((index - 1 + srcs.length) % srcs.length);
              }}
            >
              <Icon name="chevronRight" size={22} className="is-left" />
            </button>
            <button
              type="button"
              className="ilb__nav ilb__nav--next"
              aria-label={dict.ui.nextImage}
              onClick={(e) => {
                e.stopPropagation();
                onIndex((index + 1) % srcs.length);
              }}
            >
              <Icon name="chevronRight" size={22} />
            </button>
          </>
        )}
      </div>
    </div>,
    document.body,
  );
}
