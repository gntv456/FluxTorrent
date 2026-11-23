"use client";

import { useEffect, useRef } from "react";

/**
 * 刮刮乐独立中奖浮层（样图落地）：单张结算后弹出，展示图标 / 标题 /
 * 描述 / 净收金额，提供「再刮一张」与「十连」入口。纯呈现哑件，文案与
 * 金额由页面按后端结果拼好传入；自动连刮进行中页面不会打开本浮层。
 * 焦点回收 / Esc 关闭 / 遮罩点击关闭的范式与 SwTenModal 一致。
 */

export type ScratchWinKind = "win" | "lose" | "tie" | "jackpot";

const KIND_ICON: Record<ScratchWinKind, string> = {
  jackpot: "💎",
  win: "✨",
  tie: "🎫",
  lose: "🍀",
};

export function ScratchWinModal({
  open,
  kind,
  title,
  desc,
  amount,
  amountSuffix,
  againLabel,
  tenLabel,
  onAgain,
  onTen,
  onClose,
}: {
  open: boolean;
  kind: ScratchWinKind;
  title: string;
  desc: string;
  amount: string;
  amountSuffix?: string;
  againLabel: string;
  tenLabel?: string;
  onAgain: () => void;
  onTen?: () => void;
  onClose: () => void;
}) {
  const panelRef = useRef<HTMLDivElement>(null);
  useEffect(() => {
    if (!open) return;
    const prev = document.activeElement as HTMLElement | null;
    panelRef.current?.focus();
    const onKey = (e: KeyboardEvent) => {
      if (e.key === "Escape") onClose();
    };
    document.addEventListener("keydown", onKey);
    return () => {
      document.removeEventListener("keydown", onKey);
      prev?.focus?.();
    };
  }, [open, onClose]);

  if (!open) return null;

  return (
    <div className="sw-win-overlay" role="presentation" onClick={onClose}>
      <div
        ref={panelRef}
        className={`sw-win-dialog ${kind}`}
        role="dialog"
        aria-modal="true"
        aria-label={title}
        tabIndex={-1}
        onClick={(e) => e.stopPropagation()}
      >
        <button
          type="button"
          className="sw-win-x"
          onClick={onClose}
          aria-label="close"
        >
          ✕
        </button>
        <div className="sw-win-ic" aria-hidden>
          {KIND_ICON[kind]}
        </div>
        <div className={`sw-win-title ${kind}`}>{title}</div>
        {desc && <div className="sw-win-desc">{desc}</div>}
        <div className="sw-win-amt num">
          {amount}
          {amountSuffix && <small>{amountSuffix}</small>}
        </div>
        <div className="sw-win-btns">
          <button type="button" className="sw-cta arc-chip" onClick={onAgain}>
            {againLabel}
          </button>
          {tenLabel && onTen && (
            <button
              type="button"
              className="sw-cta cta-primary"
              onClick={onTen}
            >
              {tenLabel}
            </button>
          )}
        </div>
      </div>
    </div>
  );
}
