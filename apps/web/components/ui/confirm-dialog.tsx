"use client";

/** 确认对话框（DS-PLAN-01 W-16 / §3.1：替代 window.confirm 的唯一形态）。
 *
 *  - 焦点陷阱 + Esc + 遮罩点击语义（复用 lib/use-focus-trap.ts；
 *    关闭时焦点归还触发元素）；
 *  - danger 红色确认；删除类可开 requireText 二次输入
 *    （用户输入指定词才解禁确认钮，防误删）；
 *  - 遮罩点击语义按 tone 分化：neutral 可点外部取消，danger
 *    **不可**（破坏性操作的误触代价高，必须显式表态）。
 *
 *    <ConfirmDialog
 *      open={askDelete}
 *      onClose={() => setAskDelete(false)}
 *      onConfirm={doDelete}
 *      tone="danger"
 *      title="删除种子"
 *      message="删除后不可恢复，做种记录同时清除。"
 *      requireText={torrentName}
 *    />
 */

import { useEffect, useRef, useState } from "react";

import { Icon } from "@/components/icons";
import { useFocusTrap } from "@/lib/use-focus-trap";
import { useI18n } from "@/i18n/client";

export function ConfirmDialog({
  open,
  onClose,
  onConfirm,
  title,
  message,
  tone = "neutral",
  confirmLabel,
  cancelLabel,
  /** 删除类二次输入：用户需原样输入该文本才解禁确认 */
  requireText,
  busy,
}: {
  open: boolean;
  onClose: () => void;
  onConfirm: () => void;
  title: string;
  message?: string;
  tone?: "neutral" | "danger";
  confirmLabel?: string;
  cancelLabel?: string;
  requireText?: string;
  /** 确认处理中：禁用两钮，防重复提交 */
  busy?: boolean;
}) {
  const { dict } = useI18n();
  const cardRef = useRef<HTMLDivElement>(null);
  const [typed, setTyped] = useState("");
  useFocusTrap(open, cardRef);

  // Esc 关闭（焦点陷阱已管 Tab；Esc 是对话框通用退出语义）
  useEffect(() => {
    if (!open) return;
    const onKey = (e: KeyboardEvent) => {
      if (e.key === "Escape") onClose();
    };
    document.addEventListener("keydown", onKey);
    const prev = document.body.style.overflow;
    document.body.style.overflow = "hidden";
    return () => {
      document.removeEventListener("keydown", onKey);
      document.body.style.overflow = prev;
    };
  }, [open, onClose]);

  // 重开时清空二次输入（上次的输入不残留）
  useEffect(() => {
    if (open) setTyped("");
  }, [open]);

  if (!open) return null;
  const okLabel =
    confirmLabel ??
    (tone === "danger" ? dict.ui.confirmDelete : dict.ui.confirmOk);
  const textOk = !requireText || typed.trim() === requireText;

  return (
    <div
      className="ui-cfm__scrim"
      role="presentation"
      onClick={() => {
        // danger 不响应遮罩误触：必须显式点取消/确认
        if (tone !== "danger") onClose();
      }}
    >
      <div
        ref={cardRef}
        className={`ui-cfm ui-cfm--${tone}`}
        role="alertdialog"
        aria-modal
        aria-label={title}
        onClick={(e) => e.stopPropagation()}
      >
        <div className="ui-cfm__head">
          <span className="ui-cfm__icon" aria-hidden="true">
            <Icon name={tone === "danger" ? "warning" : "help"} size={18} />
          </span>
          <h2 className="ui-cfm__title">{title}</h2>
        </div>
        {message ? <p className="ui-cfm__msg">{message}</p> : null}
        {requireText ? (
          <label className="ui-cfm__type">
            <span>
              {dict.ui.confirmTypeTo.replace("{text}", requireText)}
            </span>
            <input
              value={typed}
              onChange={(e) => setTyped(e.target.value)}
              autoComplete="off"
              data-autofocus
            />
          </label>
        ) : null}
        <div className="ui-cfm__actions">
          <button
            type="button"
            className="ui-cfm__btn ui-cfm__btn--ghost"
            onClick={onClose}
            disabled={busy}
          >
            {cancelLabel ?? dict.common.cancel}
          </button>
          <button
            type="button"
            className={`ui-cfm__btn ui-cfm__btn--${tone}`}
            onClick={onConfirm}
            disabled={busy || !textOk}
          >
            {okLabel}
          </button>
        </div>
      </div>
    </div>
  );
}
