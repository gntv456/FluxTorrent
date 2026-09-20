"use client";

import type { ReactNode } from "react";

/** 发布表单共享件（从 upload-form.tsx 拆出，300 行门禁）：
 *  rowhead/rowfollow 表格行 FormRow 与输入框样式 fieldCls，
 *  供 upload-form.tsx（主表单/提交逻辑）、upload-form-files.tsx
 *  （文件与 NFO）、upload-form-descr.tsx（简介与 BBCode）、
 *  upload-form-quality.tsx（质量/标签/推荐）共用。 */

export function FormRow({
  label,
  children,
}: {
  label: string;
  children: ReactNode;
}) {
  return (
    <tr>
      <td className="rowhead">{label}</td>
      <td className="rowfollow">{children}</td>
    </tr>
  );
}

export const fieldCls =
  "min-h-[38px] w-full rounded-[var(--r-sm)] border border-[var(--baozi-line)] bg-[var(--baozi-paper)] px-3 text-sm text-ink outline-none focus:border-[var(--baozi-orange)]";
