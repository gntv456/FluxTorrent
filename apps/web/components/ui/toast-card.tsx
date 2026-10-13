"use client";

/** Toast 卡片（DS-PLAN-01 W-01，从 toast.tsx 按 300 行门禁拆出）。
 *  单条 toast 的呈现与生命周期：倒计时（悬停/聚焦暂停）、request_id
 *  一键复制、动作钮、退场动画。队列与命令式 API 在 ./toast.tsx。 */

import { useCallback, useEffect, useRef, useState } from "react";

import { Icon } from "@/components/icons";
import { useI18n } from "@/i18n/client";
import type { ToastItem, ToastTone } from "./toast";

const TONE_ICON: Record<ToastTone, string> = {
  success: "checkCircle",
  error: "warning",
  warn: "warning",
  info: "info",
};

export function ToastCard({
  item,
  onDismiss,
}: {
  item: ToastItem;
  /** 关闭回调（由队列方注入，避免 toast ↔ card 循环导入） */
  onDismiss: (id: number) => void;
}) {
  const { dict } = useI18n();
  const [leaving, setLeaving] = useState(false);
  const pausedRef = useRef(false);
  const close = useCallback(() => {
    // 退场动画 200ms 后真正移除（CSS .ui-toast--leave）
    setLeaving(true);
    window.setTimeout(() => onDismiss(item.id), 200);
  }, [item.id, onDismiss]);

  // 倒计时：悬停/键盘聚焦时暂停（pausedRef，不触发重渲染）
  useEffect(() => {
    if (item.duration <= 0) return;
    let remain = item.duration;
    const iv = window.setInterval(() => {
      if (pausedRef.current) return;
      remain -= 100;
      if (remain <= 0) {
        window.clearInterval(iv);
        close();
      }
    }, 100);
    return () => window.clearInterval(iv);
  }, [item.duration, close]);

  const [copied, setCopied] = useState(false);

  return (
    <div
      className={`ui-toast ui-toast--${item.tone}${
        leaving ? " ui-toast--leave" : ""
      }`}
      onMouseEnter={() => {
        pausedRef.current = true;
      }}
      onMouseLeave={() => {
        pausedRef.current = false;
      }}
      onFocus={() => {
        pausedRef.current = true;
      }}
      onBlur={() => {
        pausedRef.current = false;
      }}
    >
      <span className="ui-toast__icon" aria-hidden="true">
        <Icon name={TONE_ICON[item.tone]} size={16} />
      </span>
      <div className="ui-toast__body">
        <p className="ui-toast__msg">{item.message}</p>
        {item.desc ? <p className="ui-toast__desc">{item.desc}</p> : null}
        {item.requestId ? (
          <button
            type="button"
            className="ui-toast__rid"
            onClick={() => {
              void navigator.clipboard
                ?.writeText(item.requestId ?? "")
                .then(() => {
                  setCopied(true);
                  window.setTimeout(() => setCopied(false), 1500);
                })
                .catch(() => {
                  /* 剪贴板不可用（非安全上下文）时静默，文案仍在 */
                });
            }}
          >
            <Icon name="copy" size={11} />
            <span className="ui-toast__rid-label">
              {dict.ui.requestIdLabel}
            </span>
            <code className="ui-toast__rid-value">
              {copied ? dict.ui.copied : item.requestId}
            </code>
          </button>
        ) : null}
      </div>
      {item.action ? (
        <button
          type="button"
          className="ui-toast__action"
          onClick={() => {
            item.action?.onClick();
            close();
          }}
        >
          {item.action.label}
        </button>
      ) : null}
      <button
        type="button"
        className="ui-toast__close"
        aria-label={dict.ui.toastClose}
        onClick={close}
      >
        <Icon name="close" size={12} />
      </button>
    </div>
  );
}
