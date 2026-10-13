"use client";

/** 提交按钮（DS-PLAN-01 W-15 / §3.3：表单提交生命周期封装）。
 *
 *  idle → busy → done / fail 四态与 ActionFeedback 联动；防双击
 *  （busy 期间 disabled + aria-busy）；键盘提交天然支持（form 内
 *  type=submit）。状态播报走 aria-live（读屏用户知道结果）。
 *
 *    <SubmitButton busy={saving} ok={saved} label="保存" />
 */

import { ActionFeedback } from "./action-feedback";

export function SubmitButton({
  busy,
  ok,
  fail,
  label,
  busyLabel,
  okLabel,
  failLabel,
  tone = "brand",
  className,
}: {
  /** 提交中：禁用 + spinner + busyLabel */
  busy?: boolean;
  /** 成功（短暂展示后由调用方复位） */
  ok?: boolean;
  /** 失败（短暂展示后由调用方复位） */
  fail?: boolean;
  label: string;
  busyLabel?: string;
  okLabel?: string;
  failLabel?: string;
  tone?: "brand" | "danger";
  className?: string;
}) {
  const state = busy ? "busy" : ok ? "ok" : fail ? "fail" : "idle";
  const cls = `ui-submit ui-submit--${tone}${className ? ` ${className}` : ""}`;
  return (
    <button
      type="submit"
      disabled={busy}
      aria-busy={busy || undefined}
      className={cls}
    >
      {state === "idle" ? (
        label
      ) : (
        <ActionFeedback
          state={state}
          busyLabel={busyLabel ?? label}
          okLabel={okLabel ?? label}
          failLabel={failLabel ?? label}
        />
      )}
    </button>
  );
}
