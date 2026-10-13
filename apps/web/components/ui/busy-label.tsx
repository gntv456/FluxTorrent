"use client";

/** BusyLabel（DS-PLAN-01 W-02 / §3.1：文本 + spinner 组合）。
 *
 *  替代散落全站的「加载中…」手写句。进行中时 spinner 与文本同行
 *  （aria-live 由语义容器承载，本件自身是行内标注，不重复播报）；
 *  busy=false 时退化为纯文本——同一调用点在数据就绪后无需换组件。
 *
 *    <BusyLabel label={t.sending} busy={sending} />
 */

export type PrimitiveSize = "sm" | "md" | "lg";

export function BusyLabel({
  label,
  busy = true,
  size = "md",
  className,
}: {
  /** 展示文本（三语，调用方从 dict 取） */
  label: string;
  /** false 时纯文本（spinner 隐藏），默认 true */
  busy?: boolean;
  size?: PrimitiveSize;
  className?: string;
}) {
  if (!busy) return <span className={className}>{label}</span>;
  return (
    <span
      className={`ui-fb ui-fb--busy ui-fb--${size}${
        className ? ` ${className}` : ""
      }`}
    >
      <span className="ui-spinner" aria-hidden="true" />
      <span>{label}</span>
    </span>
  );
}
