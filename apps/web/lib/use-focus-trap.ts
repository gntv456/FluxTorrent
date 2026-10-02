"use client";

import { useEffect, type RefObject } from "react";

/** 焦点陷阱（ZT81 2026-10-02）。
 *
 * 此前 Modal / BottomSheet / 各弹层只有 `role="dialog"` + `aria-modal` + ESC 关闭，
 * 但**没有焦点圈定**：打开后 Tab 仍能走到遮罩后面的页面元素，也没有初始聚焦与
 * 关闭后焦点归还——键盘用户会「消失」到背景内容里，读屏也会把背景当成可交互区
 * （违反 WCAG 2.4.3 焦点顺序 / 2.1.2 无键盘陷阱）。
 *
 * 用法：给弹层根元素挂 ref，然后 `useFocusTrap(open, ref)`。
 */
const FOCUSABLE = [
  "a[href]",
  "button:not([disabled])",
  "textarea:not([disabled])",
  "input:not([disabled]):not([type=hidden])",
  "select:not([disabled])",
  "[tabindex]:not([tabindex='-1'])",
].join(",");

export function useFocusTrap(
  open: boolean,
  ref: RefObject<HTMLElement | null>,
) {
  useEffect(() => {
    if (!open) return;
    const root = ref.current;
    if (!root) return;
    // 记录打开前的焦点，关闭时归还（避免焦点掉回 body 顶部）
    const prev = document.activeElement as HTMLElement | null;
    const items = () =>
      Array.from(root.querySelectorAll<HTMLElement>(FOCUSABLE)).filter(
        (el) => el.offsetParent !== null,
      );
    // 初始聚焦：优先 data-autofocus（如输入框），否则第一个可聚焦元素
    const first =
      root.querySelector<HTMLElement>("[data-autofocus]") ?? items()[0];
    first?.focus();

    const onKey = (e: KeyboardEvent) => {
      if (e.key !== "Tab") return;
      const list = items();
      if (list.length === 0) {
        e.preventDefault();
        return;
      }
      const head = list[0];
      const tail = list[list.length - 1];
      const active = document.activeElement as HTMLElement | null;
      const outside = !active || !root.contains(active);
      if (e.shiftKey && (active === head || outside)) {
        e.preventDefault();
        tail.focus();
      } else if (!e.shiftKey && (active === tail || outside)) {
        e.preventDefault();
        head.focus();
      }
    };
    // 捕获阶段监听：先于页面其它 keydown 处理，避免被 stopPropagation 吞掉
    document.addEventListener("keydown", onKey, true);
    return () => {
      document.removeEventListener("keydown", onKey, true);
      prev?.focus?.();
    };
  }, [open, ref]);
}
