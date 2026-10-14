/** 错误对象 → 可跨 RSC 边界的普通对象（纯函数，无 "use client"）。
 *
 *  为什么单独一个文件：`error-panel.tsx` 带 `"use client"`，从服务端组件里
 *  调它导出的**函数**会抛
 *  `Attempted to call normalizeError() from the server`（10-14 实测 digest
 *  992692853），整棵子树被打断、页面只剩骨架屏。组件本身仍可从 error-panel
 *  导入——只有函数调用是禁区，故把判定逻辑放这里，两侧都能用。
 */

import { ApiError } from "@/lib/api-client";

export interface PanelError {
  code?: number | string;
  message: string;
  requestId?: string;
}

/** 任意 unknown → 可渲染的普通错误对象（ApiError 保 request_id） */
export function normalizeError(err: unknown): PanelError {
  if (err instanceof ApiError) {
    return { code: err.code, message: err.message, requestId: err.requestId };
  }
  if (err instanceof Error) return { message: err.message };
  if (typeof err === "string") return { message: err };
  return { message: String(err) };
}
