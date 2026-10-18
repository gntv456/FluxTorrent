"use client";

/** 表单协议（DS-PLAN-01 W-14 / §3.3：FormField + FieldErrors）。
 *
 *  设计意图（§10.1 宪章）：前后端校验口径单一真源——后端信封
 *  {code:1002, data:[{field, error}]} 直接渲染，不为同一规则写两套
 *  文案。FormField 管「label + 控件 + hint + error」四槽位与
 *  aria-describedby 关联；FieldErrors 管表单级汇总（顶部 alert）。
 *
 *    <FormField name="username" label="用户名" required
 *               error={errs.username} hint="3-24 字符">
 *      <input value={u} onChange={...} className={inputCls} />
 *    </FormField>
 *
 *    const errs = fieldErrorsFrom(err) ?? {};
 *    <FieldErrors fields={errs} />   // {username: ["已被占用"], ...}
 *
 *  id 规则（规格口径）：错误节点 id 固定 `field-<name>-err`，控件
 *  id 固定 `field-<name>`——同一字段两处（FieldErrors 点击聚焦与
 *  aria 关联）都拿到稳定 id，不依赖 React 渲染顺序。
 */

import {
  cloneElement,
  isValidElement,
  type ReactElement,
  type ReactNode,
} from "react";

import { Icon } from "@/components/icons";
import { ApiError } from "@/lib/api-client";
import { useI18n } from "@/i18n/client";

/** label + 控件 + hint + error 四槽位；error 非空时控件描红 */
export function FormField({
  name,
  label,
  error,
  hint,
  required,
  children,
  className,
}: {
  /** 字段名：决定控件 id（field-<name>）与错误节点 id（field-<name>-err） */
  name: string;
  label: string;
  /** 后端/前端校验错误文案（FieldErrors 解出的单字段消息） */
  error?: string;
  /** 常驻帮助文案（与 error 互斥呈现：有 error 时 hint 让位） */
  hint?: string;
  /** 必填：label 后加 * 且控件带 aria-required */
  required?: boolean;
  children: ReactNode;
  className?: string;
}) {
  const { dict } = useI18n();
  const ctlId = `field-${name}`;
  const msgId = `field-${name}-err`;
  const wrapCls = `ui-field${error ? " ui-field--err" : ""}${
    className ? ` ${className}` : ""
  }`;
  // 把 id / aria-describedby / aria-required 织进调用方的控件（单元素
  // 子节点约定）——label 关联与错误播报都不需要调用方手写 id。
  // aria-* 只给可聚焦控件（input/select/textarea/button…）：包一层
  // 布局 div 的调用形态（验证码题面+输入框）会把 aria-required 落到
  // div 上，axe 的 aria-allowed-attr 直接拦（Lighthouse a11y 实测过）。
  const isControl = (el: ReactElement<Record<string, unknown>>): boolean => {
    const t = String(el.props?.role ?? el.type);
    return t === "input" || t === "select" || t === "textarea" || t === "button";
  };
  const ctl = isValidElement(children)
    ? cloneElement(children as ReactElement<Record<string, unknown>>, {
        id: ctlId,
        "aria-describedby": error || hint ? msgId : undefined,
        "aria-invalid": error ? true : undefined,
        "aria-required":
          required &&
          isControl(children as ReactElement<Record<string, unknown>>)
            ? true
            : undefined,
      })
    : children;
  return (
    <div className={wrapCls}>
      <label className="ui-field__label" htmlFor={ctlId}>
        {label}
        {required ? (
          <span className="ui-field__req" aria-hidden="true">
            *
          </span>
        ) : (
          <span className="ui-field__opt">{dict.ui.fieldOptional}</span>
        )}
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

/** 字段错误 map（field -> msgs）。后端信封解出的形态（见 fieldsFrom） */
export type FieldErrorMap = Record<string, string | string[]>;

/** 表单级汇总：顶部 alert 列出全部字段错误（点字段名聚焦对应控件）。
 *  与后端 1002 信封对齐：data 是 [{field, error}] 数组。 */
export function FieldErrors({
  fields,
  className,
}: {
  /** 字段级错误（FieldErrors 信封解出，或 ApiError.fields 直用） */
  fields: FieldErrorMap;
  className?: string;
}) {
  const { dict } = useI18n();
  const entries = Object.entries(fields).filter(([, v]) =>
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
            <button
              type="button"
              className="ui-ferr__field"
              // 点击聚焦对应控件（id 规则 field-<name>；FieldErrors 顶部
              // 汇总与字段本体可能隔着长表单，键盘/点击直达）
              onClick={() =>
                document.getElementById(`field-${field}`)?.focus()
              }
            >
              {field}
            </button>
            <span className="ui-ferr__msg">
              {Array.isArray(v) ? v.join("；") : v}
            </span>
          </li>
        ))}
      </ul>
    </div>
  );
}

/** 后端 1002 信封 data（[{field,error}] 或 {field:[msgs]}）→ FieldErrorMap */
export function fieldsFrom(data: unknown): FieldErrorMap {
  const out: FieldErrorMap = {};
  if (Array.isArray(data)) {
    for (const item of data as { field?: string; error?: string }[]) {
      if (!item?.field || !item.error) continue;
      const prev = out[item.field];
      out[item.field] = Array.isArray(prev)
        ? [...prev, item.error]
        : prev
          ? [prev, item.error]
          : item.error;
    }
  } else if (data && typeof data === "object") {
    for (const [f, v] of Object.entries(
      data as Record<string, string | string[]>,
    )) {
      if (v) out[f] = v;
    }
  }
  return out;
}

/** ApiError → FieldErrorMap；无字段错误时返回 null。
 *  兼容旧签名（1002 数组信封）与新形态（ApiError.fields getter）。 */
export function fieldErrorsFrom(e: unknown): FieldErrorMap | null {
  if (!(e instanceof ApiError)) return null;
  if (e.fields) return e.fields;
  if (e.code !== 1002 || !Array.isArray(e.data)) return null;
  const out = fieldsFrom(e.data);
  return Object.keys(out).length > 0 ? out : null;
}
