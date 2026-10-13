"use client";

/** 表单协议（DS-PLAN-01 W-14 / §3.3：FormField + FieldErrors）。
 *
 *  设计意图（§10.1 宪章）：前后端校验口径单一真源——后端信封
 *  {code:1002, data:[{field, error}]} 直接渲染，不为同一规则写两套
 *  文案。FormField 管「label + 控件 + hint + error」四槽位与
 *  aria-describedby 关联；FieldErrors 管表单级汇总（顶部 alert）。
 *
 *    <FormField label="用户名" required error={errs.username} hint="3-24 字符">
 *      <input value={u} onChange={...} className={inputCls} />
 *    </FormField>
 *
 *    <FieldErrors errors={errs} />   // {username: "已被占用", ...}
 */

import {
  cloneElement,
  isValidElement,
  useId,
  type ReactElement,
  type ReactNode,
} from "react";

import { Icon } from "@/components/icons";
import { ApiError } from "@/lib/api-client";
import { useI18n } from "@/i18n/client";

/** label + 控件 + hint + error 四槽位；error 非空时控件描红 */
export function FormField({
  label,
  error,
  hint,
  required,
  children,
  className,
}: {
  label: string;
  /** 后端/前端校验错误文案（FieldErrors 解出的单字段消息） */
  error?: string;
  /** 常驻帮助文案（与 error 互斥呈现：有 error 时 hint 让位） */
  hint?: string;
  required?: boolean;
  children: ReactNode;
  className?: string;
}) {
  const { dict } = useI18n();
  const uid = useId();
  const msgId = `${uid}-msg`;
  const wrapCls = `ui-field${error ? " ui-field--err" : ""}${
    className ? ` ${className}` : ""
  }`;
  // 把 id / aria-describedby 织进调用方的控件（单元素子节点约定）——
  // label 关联与错误播报都不需要调用方手写 id
  const ctl = isValidElement(children)
    ? cloneElement(children as ReactElement<Record<string, unknown>>, {
        id: uid,
        "aria-describedby": error || hint ? msgId : undefined,
        "aria-invalid": error ? true : undefined,
      })
    : children;
  return (
    <div className={wrapCls}>
      <label className="ui-field__label" htmlFor={uid}>
        {label}
        {required ? (
          <span className="ui-field__req" aria-hidden="true">
            *
          </span>
        ) : null}
        {!required ? (
          <span className="ui-field__opt">{dict.ui.fieldOptional}</span>
        ) : null}
      </label>
      <div className="ui-field__ctl">{ctl}</div>
      {error ? (
        <p className="ui-field__err" id={msgId}>
          <Icon name="warning" size={11} />
          {error}
        </p>
      ) : hint ? (
        <p className="ui-field__hint" id={msgId}>
          {hint}
        </p>
      ) : null}
    </div>
  );
}

/** 字段错误 map（field -> message）。兼容单值与多值。 */
export type FieldErrorMap = Record<string, string | string[]>;

/** 表单级汇总：顶部 alert 列出全部字段错误（点字段名聚焦对应控件）。
 *  与后端 1002 信封对齐：data 是 [{field, error}] 数组。 */
export function FieldErrors({
  errors,
  className,
}: {
  errors: FieldErrorMap;
  className?: string;
}) {
  const { dict } = useI18n();
  const entries = Object.entries(errors).filter(([, v]) =>
    Array.isArray(v) ? v.length > 0 : Boolean(v),
  );
  if (entries.length === 0) return null;
  return (
    <div
      className={`ui-ferr${className ? ` ${className}` : ""}`}
      role="alert"
      aria-live="assertive"
    >
      <p className="ui-ferr__title">
        <Icon name="warning" size={13} />
        {dict.ui.fieldErrorsTitle}
      </p>
      <ul className="ui-ferr__list">
        {entries.map(([field, v]) => (
          <li key={field}>
            <span className="ui-ferr__field">{field}</span>
            <span className="ui-ferr__msg">
              {Array.isArray(v) ? v.join("；") : v}
            </span>
          </li>
        ))}
      </ul>
    </div>
  );
}

/** ApiError(1002) → FieldErrorMap；非字段错误返回 null */
export function fieldErrorsFrom(e: unknown): FieldErrorMap | null {
  if (!(e instanceof ApiError) || e.code !== 1002) return null;
  if (!Array.isArray(e.data)) return null;
  const out: FieldErrorMap = {};
  for (const item of e.data as { field?: string; error?: string }[]) {
    if (!item?.field || !item.error) continue;
    const prev = out[item.field];
    out[item.field] = Array.isArray(prev)
      ? [...prev, item.error]
      : prev
        ? [prev, item.error]
        : item.error;
  }
  return Object.keys(out).length > 0 ? out : null;
}
