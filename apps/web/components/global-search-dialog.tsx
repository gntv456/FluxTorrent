"use client";

import { useEffect, useRef, useState } from "react";
import { createPortal } from "react-dom";

import { GlobalSearch } from "@/components/global-search";
import { Icon } from "@/components/icons";
import { Modal } from "@/components/modal";

interface Labels {
  /** 按钮与弹窗标题（aria-label 同源，避免两处措辞分叉） */
  label: string;
  placeholder: string;
  tipTorrents: string;
  tipTopics: string;
  tipEmpty: string;
  tipMore: string;
}

/**
 * 头栏搜索：只留一个按钮，点开弹窗（站长 2026-10-08 提的口径）。
 *
 * 换掉的是「搜索框独占头栏一行」的老结构：实测两行头栏 117px、一行 73px，
 * 且 1024 档不必再回退（把搜索挤进右工具区时，1024 会在卡片内换行成 109px）。
 * 弹窗复用共用 `Modal`（焦点圈定 / ESC / 点遮罩关闭 / 锁 body 滚动），
 * 搜索本体仍是 `GlobalSearch` 那套「种子 + 论坛」双路联想，判据没有第二份。
 */
export function GlobalSearchDialog(t: Labels) {
  const [open, setOpen] = useState(false);
  const [mounted, setMounted] = useState(false);
  const host = useRef<HTMLDivElement>(null);

  useEffect(() => setMounted(true), []);

  // 打开即聚焦输入框：Modal 的焦点圈定只把焦点收到卡片上，
  // 不主动聚焦就得多点一下才能打字，搜索弹窗就白做了
  useEffect(() => {
    if (!open) return;
    const el = host.current?.querySelector<HTMLInputElement>("input");
    el?.focus();
  }, [open]);

  const dialog = (
    <Modal open={open} onClose={() => setOpen(false)} title={t.label} size="md">
      <div ref={host} className="gsearch--dlg">
        <GlobalSearch
          placeholder={t.placeholder}
          tipTorrents={t.tipTorrents}
          tipTopics={t.tipTopics}
          tipEmpty={t.tipEmpty}
          tipMore={t.tipMore}
        />
      </div>
    </Modal>
  );

  return (
    <>
      <button
        type="button"
        className="gsearch-btn"
        onClick={() => setOpen(true)}
        aria-haspopup="dialog"
        aria-label={t.label}
        title={t.label}
      >
        <Icon name="search" size={16} />
        <span className="gsearch-btn__text">{t.label}</span>
      </button>
      {/* 必须挂到 body：头栏 `.tide-header` 带 `backdrop-filter`
        （theme-tide.css:468），而 backdrop-filter 会让该祖先成为
        `position: fixed` 的包含块——弹窗留在头栏里时，遮罩只盖住头栏那
        73px、联想列表被弹层容器裁掉（DOM 里元素都在，屏幕上看不见）。 */}
      {mounted && open ? createPortal(dialog, document.body) : null}
    </>
  );
}
