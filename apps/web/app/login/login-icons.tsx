"use client";

/** 登录表单 SVG 图标（从 login-form.tsx 拆出，行数门禁）。
 *  theme.css：stroke 1.8 / round，25×25 左侧嵌入输入框。 */

/** SVG 图标（theme.css：stroke 1.8 / round，25×25 左侧嵌入输入框） */
export function IconUser() {
  return (
    <svg viewBox="0 0 24 24" aria-hidden>
      <circle cx="12" cy="8" r="3.6" />
      <path d="M4.8 20c1.2-3.6 4-5.4 7.2-5.4s6 1.8 7.2 5.4" />
    </svg>
  );
}
export function IconLock() {
  return (
    <svg viewBox="0 0 24 24" aria-hidden>
      <rect x="5" y="10.5" width="14" height="9" rx="2" />
      <path d="M8.2 10.5V8a3.8 3.8 0 0 1 7.6 0v2.5" />
      <circle cx="12" cy="15" r="1.3" />
    </svg>
  );
}
export function IconShield() {
  return (
    <svg viewBox="0 0 24 24" aria-hidden>
      <path d="M12 3.5 19 6v5.4c0 4.4-2.9 7.6-7 9.1-4.1-1.5-7-4.7-7-9.1V6z" />
      <path d="m9.2 12 2 2 3.6-3.8" />
    </svg>
  );
}

