"use client";

import { useEffect, useState } from "react";
import { useI18n } from "@/i18n/client";

/**
 * 列表页键盘快捷键（方案阶段二「最顺手」）——PT 站与 GitHub 同套习惯：
 *   /      聚焦搜索框（输入态不触发）
 *   j / k  在可导航条目上/下移动（表格行、卡片、海报墙皆可）
 *   Enter  打开当前高亮条目
 *   Esc    清除高亮并失焦
 *
 * 可导航单元统一用 `[data-torrent-id]`（三种视图都带该属性）——这同时是
 * 竞品调研里被打分最高的「DOM 语义契约」：油猴脚本可据此零成本适配。
 * 打字时（input/textarea/select/contenteditable）不拦截 j/k，避免吃掉输入。
 */
export function TorrentsHotkeys() {
  const { dict } = useI18n();
  const [showHint, setShowHint] = useState(false);

  useEffect(() => {
    const items = () =>
      Array.from(document.querySelectorAll<HTMLElement>("[data-torrent-id]"));
    let idx = -1;

    const clear = () => {
      for (const el of items()) el.classList.remove("is-kbd-active");
      idx = -1;
    };
    const move = (step: number) => {
      const list = items();
      if (!list.length) return;
      clear();
      idx = (idx + step + list.length) % list.length;
      const el = list[idx];
      el.classList.add("is-kbd-active");
      el.scrollIntoView({ block: "nearest" });
    };
    const open = () => {
      const el = items()[idx];
      if (!el) return;
      const a =
        el instanceof HTMLAnchorElement
          ? el
          : el.querySelector<HTMLAnchorElement>('a[href^="/torrent/"]');
      a?.click();
    };
    const onKey = (e: KeyboardEvent) => {
      const t = e.target as HTMLElement | null;
      const typing =
        !!t &&
        (t.tagName === "INPUT" ||
          t.tagName === "TEXTAREA" ||
          t.tagName === "SELECT" ||
          t.isContentEditable);
      if (e.key === "Escape") {
        clear();
        (document.activeElement as HTMLElement | null)?.blur?.();
        return;
      }
      if (e.key === "/" && !typing) {
        const box = document.getElementById("searchinput");
        if (box instanceof HTMLInputElement) {
          e.preventDefault();
          box.focus();
          box.select();
        }
        return;
      }
      if (typing || e.metaKey || e.ctrlKey || e.altKey) return;
      if (e.key === "j") {
        e.preventDefault();
        move(1);
      } else if (e.key === "k") {
        e.preventDefault();
        move(-1);
      } else if (e.key === "Enter" && idx >= 0) {
        e.preventDefault();
        open();
      }
    };
    setShowHint(true);
    const timer = setTimeout(() => setShowHint(false), 8000);
    document.addEventListener("keydown", onKey);
    return () => {
      document.removeEventListener("keydown", onKey);
      clearTimeout(timer);
      clear();
    };
  }, []);

  if (!showHint) return null;
  const t = dict.torrents;
  return (
    <p className="tsb-hotkeys" role="status">
      <kbd>/</kbd> {t.hkSearch}
      <span aria-hidden>·</span>
      <kbd>j</kbd>
      <kbd>k</kbd> {t.hkMove}
      <span aria-hidden>·</span>
      <kbd>Enter</kbd> {t.hkOpen}
    </p>
  );
}
