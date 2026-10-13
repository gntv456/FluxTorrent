"use client";

/** 行内反馈三态（DS-PLAN-01 W-02 / §3.1）。
 *
 *  替代现状：各表单 20+ 处手写「提交中…」——文案散落、无图标语义、
 *  无 a11y 状态播报。本件把 busy / ok / fail 收敛为 tone×size 矩阵内的
 *  原语：
 *
 *    <ActionFeedback state="busy" />                 // 处理中…
 *    <ActionFeedback state="ok" okLabel="已保存" />   // ✓ 已保存
 *    <BusyLabel label="发送中" />                    // spinner + 文本
 *
 *  ActionFeedback 状态变化由 aria-live="polite" 播报（读屏用户知道
 *  操作结果）；ok 态绿色闪现 1.5s 后自动归 idle（调用方无需复位），
 *  fail 态红色持续（错误需要用户处理，不自动消失）；
 *  BusyLabel 移至 ./busy-label.tsx（本文件按行宽门禁瘦身）。
 *  默认文案走 dict.ui（三语），调用方可覆盖。
 *
 *  手测说明（三态）：
 *  1. busy —— 12px spinner（ui.css @keyframes ui-spin，reduced-motion
 *     下被全局短路停转）+ 默认「处理中…」；
 *  2. ok   —— 绿色 ✓ + 默认「完成」，1.5s 后淡出归 idle；
 *  3. fail —— 红色 ✗ + 默认「失败」，常驻直至调用方切换状态。
 */

import { useEffect, useState } from "react";

import { Icon } from "@/components/icons";
import { useI18n } from "@/i18n/client";
import { BusyLabel, type PrimitiveSize } from "./busy-label";

export { BusyLabel };
export type { PrimitiveSize };

const SIZE_PX: Record<PrimitiveSize, number> = { sm: 12, md: 13, lg: 14 };

/** ok 态展示时长：1.5s 闪现后归 idle（DS-PLAN-01 §3.1 口径） */
const OK_LINGER_MS = 1500;

/** 行内三态：busy（spinner）/ ok（✓）/ fail（✗）；idle 不渲染。 */
export function ActionFeedback({
  state,
  size = "md",
  busyLabel,
  okLabel,
  failLabel,
  className,
}: {
  state: "idle" | "busy" | "ok" | "fail";
  size?: PrimitiveSize;
  busyLabel?: string;
  okLabel?: string;
  failLabel?: string;
  className?: string;
}) {
  const { dict } = useI18n();
  // ok 态计时：外部 state 保持 "ok" 期间只展示 1.5s，随后本件自行
  // 归隐（不回写调用方 state——反馈件不该反向驱动表单状态机）。
  const [okHidden, setOkHidden] = useState(false);
  useEffect(() => {
    setOkHidden(false);
    if (state !== "ok") return;
    const t = window.setTimeout(() => setOkHidden(true), OK_LINGER_MS);
    return () => window.clearTimeout(t);
  }, [state]);

  if (state === "idle" || (state === "ok" && okHidden)) return null;
  const px = SIZE_PX[size];
  return (
    <span
      className={`ui-fb ui-fb--${state} ui-fb--${size}${
        className ? ` ${className}` : ""
      }`}
      aria-live="polite"
    >
      {state === "busy" ? (
        <>
          <span className="ui-spinner" aria-hidden="true" />
          <span>{busyLabel ?? dict.ui.actionFeedbackBusy}</span>
        </>
      ) : state === "ok" ? (
        <>
          <Icon name="check" size={px} />
          <span>{okLabel ?? dict.ui.actionFeedbackOk}</span>
        </>
      ) : (
        <>
          <Icon name="close" size={px} />
          <span>{failLabel ?? dict.ui.actionFeedbackFail}</span>
        </>
      )}
    </span>
  );
}
