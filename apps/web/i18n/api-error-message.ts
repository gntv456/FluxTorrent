/** 错误消息：优先按 code 查字典（与后端 Accept-Language 协商互为兜底）。
 *
 *  2026-11-11 瘦身批从 i18n/client.tsx 搬出：本函数是纯函数（只依赖
 *  ApiError 的形状），RSC 服务端组件也能直接 import，不必再拖整本
 *  "use client" 模块。i18n/client.tsx re-export 保住既有 191 处 import。 */

import type { Dict } from "./zh-CN";
import { ApiError } from "@/lib/api-client";

export function apiErrorMessage(dict: Dict, err: unknown): string {
  if (err instanceof ApiError) {
    // P-117 → P-305：不止 1002。后端把字典标签拼在前面再挂具体判据
    // （「参数校验失败: 还款金额必须为正」「种子文件无效: 未标记私有…」），
    // 旧写法拿字典那句盖掉它，用户只知道「失败了」。判据改成通用形状：
    // message 以本码的字典标签开头且冒号后还有正文 ⇒ 展示正文。
    const code = err.code;
    const label = dict.errors?.[code];
    const raw = err.message ?? "";
    if (label && raw.startsWith(label)) {
      const tail = raw
        .slice(label.length)
        .replace(/^[:：]\s*/, "")
        .trim();
      // 尾巴还得是人话：P-304 之前 1002 挂的是 serde 原文（英文内部诊断），
      // 现在源头已修，但这条判据留着——以后再有链路漏原文也不会摊到界面上
      if (tail && /[一-鿿]{3,}/.test(tail)) return tail;
    }
    return label ?? raw;
  }
  if (err && typeof err === "object" && "code" in err) {
    // 手搓 fetch 的站点（上传、部分管理面板）拿到的是裸信封而不是 ApiError，
    // 同一套规则也要管用（P-303 的 rejectReason 与此合并成一条口径）
    return apiErrorMessage(
      dict,
      new ApiError(
        Number((err as { code?: unknown }).code),
        String((err as { message?: unknown }).message ?? ""),
        (err as { data?: unknown }).data,
      ),
    );
  }
  return err instanceof Error ? err.message : dict.common.networkError;
}
