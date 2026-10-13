"use client";

/** ToastProvider + 视口（DS-PLAN-01 W-01，从 toast.tsx 按行宽门禁拆出）。
 *
 *  根布局挂载一次（app/layout.tsx）：children 之后渲染 viewport，
 *  任意深度可编程式调用 toast.*（队列与命令式 API 在 ./toast.tsx）。
 *  不包 context——toast 是命令式 API，不需要在渲染树里传递。
 *
 *  双 live region：polite（role="status"）承载 success/info/warn，
 *  assertive（role="alert"）独立承载 error——同一 region 混排时
 *  assertive 会被 polite 降级延迟（a11y 实践）。
 *
 *  键盘：Esc 关闭最新一条（viewport 级监听，无需聚焦到卡片；
 *  单条卡片上的 ✕ 按钮是逐条关闭的指针通道）。
 */

import { useEffect } from "react";

import { toast, useToastQueue } from "./toast";
import { ToastCard } from "./toast-card";

export function ToastProvider({ children }: { children: React.ReactNode }) {
  const list = useToastQueue();

  // Esc 关最新单条：挂在 window（capture 阶段不必——toast 无嵌套），
  // 空队列时零开销返回。useEffect 依赖 list 保证闭包里的 list 最新。
  useEffect(() => {
    if (list.length === 0) return;
    const onKey = (e: KeyboardEvent) => {
      if (e.key !== "Escape") return;
      const last = list[list.length - 1];
      if (last) toast.dismiss(last.id);
    };
    window.addEventListener("keydown", onKey);
    return () => window.removeEventListener("keydown", onKey);
  }, [list]);

  return (
    <>
      {children}
      <div
        className="ui-toast__vp"
        role="status"
        aria-live="polite"
        aria-atomic="false"
      >
        {list
          .filter((t) => t.tone !== "error")
          .map((t) => (
            <ToastCard key={t.id} item={t} onDismiss={toast.dismiss} />
          ))}
      </div>
      <div
        className="ui-toast__vp"
        role="alert"
        aria-live="assertive"
        aria-atomic="false"
      >
        {list
          .filter((t) => t.tone === "error")
          .map((t) => (
            <ToastCard key={t.id} item={t} onDismiss={toast.dismiss} />
          ))}
      </div>
    </>
  );
}
