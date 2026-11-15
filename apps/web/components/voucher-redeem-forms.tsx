"use client";

/** 券核销表单三件套（voucher-panel 拆件，300 行门禁）：
 *  ConfirmPair 确认/取消按钮对；RedeemForm 核销输入（种子 ID /
 *  新用户名两种核销共用）；StartButton 进入核销态的起始按钮。 */

import { useI18n } from "@/i18n/client";
import { INPUT_MD } from "@/lib/ui-classes";

/** 核销确认/取消按钮对（三种券核销表单共用） */
export function ConfirmPair({
  onConfirm,
  onCancel,
}: {
  onConfirm: () => void;
  onCancel: () => void;
}) {
  const { dict } = useI18n();
  return (
    <div className="flex gap-2">
      <button
        type="button"
        className="btn btn-sm btn-primary"
        onClick={onConfirm}
      >
        {dict.vouchers.confirm}
      </button>
      <button type="button" className="btn btn-sm btn-ghost" onClick={onCancel}>
        {dict.vouchers.cancel}
      </button>
    </div>
  );
}

/** 核销输入表单（种子 ID / 新用户名两种核销共用） */
export function RedeemForm({
  placeholder,
  value,
  onChange,
  onConfirm,
  onCancel,
  inputMode,
  maxLength,
}: {
  placeholder: string;
  value: string;
  onChange: (v: string) => void;
  onConfirm: () => void;
  onCancel: () => void;
  inputMode?: "numeric" | "text";
  maxLength?: number;
}) {
  return (
    <div className="flex flex-col gap-2">
      <input
        className={`${INPUT_MD} w-full text-sm`}
        placeholder={placeholder}
        value={value}
        maxLength={maxLength}
        inputMode={inputMode}
        onChange={(e) => onChange(e.target.value)}
      />
      <ConfirmPair onConfirm={onConfirm} onCancel={onCancel} />
    </div>
  );
}

/** 进入核销态的起始按钮 */
export function StartButton({
  label,
  onStart,
}: {
  label: string;
  onStart: () => void;
}) {
  return (
    <button type="button" className="btn btn-sm btn-primary" onClick={onStart}>
      {label}
    </button>
  );
}
