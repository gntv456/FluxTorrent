"use client";

import { useRef, useState } from "react";

import { Icon } from "@/components/icons";
import { ImageLightbox } from "@/components/image-lightbox";
import { useI18n } from "@/i18n/client";

/** 简介内的可放大图片：缩略图受限（免得 1920px 截图糊满整屏），
 *  点击进浮层看原图。缩略图尺寸走 CSS 令牌，这里只管行为。 */
export function ZoomImage({ src }: { src: string }) {
  const { dict } = useI18n();
  const btnRef = useRef<HTMLButtonElement>(null);
  const [list, setList] = useState<string[] | null>(null);
  const [idx, setIdx] = useState(0);
  const [bad, setBad] = useState(false);

  // 画廊取同一段落内的全部图片：作用域由 RichText 的 data-rt-gallery
  // 给出；脱离该作用域（如 wiki 页）则本图独占一个列表。
  const open = () => {
    const btn = btnRef.current;
    const scope = btn?.closest("[data-rt-gallery]") ?? btn;
    const imgs = scope
      ? Array.from(
          scope.querySelectorAll<HTMLImageElement>("img[data-rt-img]"),
        )
      : [];
    const own =
      btn?.querySelector<HTMLImageElement>("img[data-rt-img]") ?? null;
    const at = own ? imgs.indexOf(own) : 0;
    const srcs = imgs.map((i) => i.currentSrc || i.src);
    setList(srcs.length > 0 ? srcs : [src]);
    setIdx(at >= 0 ? at : 0);
  };

  return (
    <>
      <button
        type="button"
        ref={btnRef}
        className="rt-zoom"
        onClick={open}
        aria-label={dict.ui.zoomImage}
        title={dict.ui.zoomImage}
      >
        <img
          src={src}
          alt=""
          loading="lazy"
          decoding="async"
          data-rt-img=""
          className={bad ? "rt-zoom__img is-bad" : "rt-zoom__img"}
          onError={() => setBad(true)}
        />
        {bad && (
          <span className="rt-zoom__bad">{dict.ui.imageFailed}</span>
        )}
        <span
          className="rt-zoom__badge bg-black/55 text-white"
          aria-hidden="true"
        >
          <Icon name="search" size={14} />
        </span>
      </button>
      {list && (
        <ImageLightbox
          srcs={list}
          index={idx}
          onIndex={setIdx}
          onClose={() => setList(null)}
        />
      )}
    </>
  );
}
