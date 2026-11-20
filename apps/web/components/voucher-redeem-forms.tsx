"use client";

/** 券核销表单三件套（voucher-panel 拆件，300 行门禁）：
 *  ConfirmPair 确认/取消按钮对；RedeemForm 核销输入（种子 ID /
 *  新用户名两种核销共用）；StartButton 进入核销态的起始按钮。 */

import { useI18n } from "@/i18n/client";
import { SubmitButton } from "@/components/ui/submit-button";
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
  const { dict } = useI18n();
  return (
    <div className="flex flex-col gap-2">
      <input
        className={`${INPUT_MD} w-full text-sm`}
        placeholder={placeholder}
        value={value}
        maxLength={maxLength}
        inputMode={inputMode}
        aria-label={placeholder}
        onChange={(e) => onChange(e.target.value)}
      />
      {/* ID 从哪来此前完全没有提示，用户得跳出去找再手抄回来 */}
      {inputMode === "numeric" && (
        <a href="/my/torrentlist" className="text-xs text-link underline">
          {dict.review.voucherSelectTorrent} →
        </a>
      )}
      <ConfirmPair onConfirm={onConfirm} onCancel={onCancel} />
    </div>
  );
}

/** 进入核销态的起始按钮（W-15：提交生命周期走 SubmitButton 原语，
 *  busy/ok/fail 态与防双击由原语封装；type/onClick 经 rest 透传） */
export function StartButton({
  label,
  onStart,
  busy,
  ok,
  fail,
}: {
  label: string;
  onStart: () => void;
  busy?: boolean;
  ok?: boolean;
  fail?: boolean;
}) {
  return (
    <SubmitButton
      busy={busy}
      ok={ok}
      fail={fail}
      label={label}
      onClick={onStart}
      type="button"
      className="btn-sm"
    />
  );
}
