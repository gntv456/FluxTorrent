"use client";

import { useEffect, useState } from "react";
import { Icon } from "@/components/icons";

/**
 * 右下角「至顶端 / 至底端」浮动按钮。
 *
 * 行为约定：
 *  - 滚动超过 240px 才出现（短页面无滚动，按钮没有意义且会遮挡内容）；
 *  - 「至底端」在已经到底时禁用（视觉 40% + not-allowed），避免无反馈的点击；
 *  - 尊重 prefers-reduced-motion：改为瞬时跳转，不做平滑滚动；
 *  - 层级 z-[45]：高于移动端底部 TabBar（z-40）、低于顶栏下拉（z-60）与弹窗（z-90）；
 *  - 位置：移动端 bottom-20 抬离 TabBar（约 60px 高 + 视觉间距），桌面 bottom-6。
 *
 * 颜色一律用主题语义变量（`--surface-card`/`--border-*`/`--sky-*`），
 * 因此浅色与夜间主题下都成立（不引用只在浅色定义的 --tide-* 专有令牌）。
 */
export function ScrollButtons() {
  const [show, setShow] = useState(false);
  const [atBottom, setAtBottom] = useState(false);

  useEffect(() => {
    const read = () => {
      const doc = document.documentElement;
      const y = window.scrollY;
      const max = doc.scrollHeight - window.innerHeight;
      setShow(y > 240);
      setAtBottom(max > 0 && max - y < 24);
    };
    read();
    window.addEventListener("scroll", read, { passive: true });
    window.addEventListener("resize", read);
    return () => {
      window.removeEventListener("scroll", read);
      window.removeEventListener("resize", read);
    };
  }, []);

  const jump = (top: number) => {
    const reduce =
      typeof window.matchMedia === "function" &&
      window.matchMedia("(prefers-reduced-motion: reduce)").matches;
    window.scrollTo({ top, behavior: reduce ? "auto" : "smooth" });
  };

  const btnClass =
    "flex h-10 w-10 items-center justify-center rounded-[8px] border border-[var(--border-soft)] " +
    "bg-[var(--surface-card)] text-[var(--text-muted)] shadow-[var(--shadow-card)] " +
    "transition-[background-color,border-color,color,transform,opacity] duration-150 " +
    "hover:border-[var(--border-deep)] hover:bg-[var(--sky-soft)] hover:text-[var(--sky-deep)] " +
    "active:scale-95 disabled:cursor-not-allowed disabled:opacity-40";

  return (
    <div
      className={`fixed right-4 bottom-20 z-[45] flex flex-col gap-2 transition-opacity duration-200 md:right-6 md:bottom-6 ${
        show ? "opacity-100" : "pointer-events-none opacity-0"
      }`}
      aria-hidden={!show}
    >
      <button
        type="button"
        className={btnClass}
        onClick={() => jump(0)}
        aria-label="回到顶部"
        title="回到顶部"
        tabIndex={show ? 0 : -1}
      >
        <Icon name="arrowUp" size={18} />
      </button>
      <button
        type="button"
        className={btnClass}
        onClick={() => jump(document.documentElement.scrollHeight)}
        disabled={atBottom}
        aria-label="跳到页面底部"
        title="跳到页面底部"
        tabIndex={show ? 0 : -1}
      >
        <Icon name="arrowDown" size={18} />
      </button>
    </div>
  );
}

export default ScrollButtons;
