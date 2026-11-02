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
  className,
}: {
  /** 文本或节点（维度行的必填星号等内联标记走节点）；
   *  省略 = 只给内容（窄屏折叠按钮那种整行只用一格的场合） */
  label?: ReactNode;
  children: ReactNode;
  /** 附加类名（窄屏折叠分组用，见 .uf-opt / .uf-more-toggle） */
  className?: string;
}) {
  return (
    <tr className={className}>
      <td className="rowhead">{label}</td>
      <td className="rowfollow">{children}</td>
    </tr>
  );
}

export const fieldCls =
  "min-h-10 w-full rounded-[var(--r-sm)] border border-[var(--brand-line)] bg-[var(--brand-paper)] px-3 text-sm text-ink outline-none focus:border-[var(--brand-orange)]";
