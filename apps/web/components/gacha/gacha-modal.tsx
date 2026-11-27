"use client";

/**
 * /gacha 弹窗骨架（2026-11-27 从 gacha-stage.tsx 拆出守行数门禁）：
 * scrim + 卡片壳 + 关闭钮 + 底部按钮条；Escape 关闭由本件挂监听。
 * 纯展示件——数据与开合状态全部由调用方（gacha-stage）下发。
 */
import { useEffect } from "react";
import type { ModalKind } from "./stage-types";

export function Modal({
  open,
  kind,
  title,
  onClose,
  closeLabel,
  closeText,
  children,
  footer,
}: {
  open: boolean;
  kind: ModalKind;
  title: string;
  onClose: () => void;
  closeLabel: string;
  closeText: string;
  children: React.ReactNode;
  footer?: React.ReactNode;
}) {
  useEffect(() => {
    if (!open) return;
    const onKey = (e: KeyboardEvent) => {
      if (e.key === "Escape") onClose();
    };
    window.addEventListener("keydown", onKey);
    return () => window.removeEventListener("keydown", onKey);
  }, [open, onClose]);
  return (
    <div className={`modal${open ? " on" : ""} mk-${kind}`} aria-hidden={!open}>
      <div className="modal-scrim" onClick={onClose} />
      <div
        className="modal-card"
        role="dialog"
        aria-modal="true"
        aria-label={title}
      >
        <div className="modal-hd">
          <div>
            <span className="m-k">Astral Archive</span>
            <b className="m-title">{title}</b>
          </div>
          <button
            type="button"
            className="modal-x"
            onClick={onClose}
            aria-label={closeLabel}
          >
            <svg
              width="12"
              height="12"
              viewBox="0 0 24 24"
              stroke="currentColor"
              fill="none"
              strokeWidth="1.7"
            >
              <path d="M6 6l12 12M18 6l-12 12" />
            </svg>
          </button>
        </div>
        <div className="modal-bd">{children}</div>
        <div className="modal-ft">
          {footer ?? (
            <button
              type="button"
              className="btn btn-ghost ft-know"
              onClick={onClose}
            >
              {closeText}
            </button>
          )}
        </div>
      </div>
    </div>
  );
}
