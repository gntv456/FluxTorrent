"use client";

/**
 * 客户端 i18n：LocaleProvider 注入整本字典（gzip 后数 KB），useI18n() 消费。
 * dict 内文案中的「火花」占位符 {spark}（源字典写作 {magic}）会按站点货币名
 * 动态替换（默认「魔力」，站长可在后台 site_settings.currency_name 改任意名），
 * 所以 useI18n() 额外暴露 fmt() 之外的 currency 名。
 */

import { createContext, useContext } from "react";
import type { Dict } from "./zh-CN";
import type { Locale } from "./config";
import { ApiError } from "@/lib/api-client";

const I18nContext = createContext<{
  dict: Dict;
  locale: Locale;
  currency: string;
} | null>(null);

export function LocaleProvider({
  dict,
  locale,
  currency = "魔力",
  children,
}: {
  dict: Dict;
  locale: Locale;
  currency?: string;
  children: React.ReactNode;
}) {
  return (
    <I18nContext.Provider value={{ dict, locale, currency }}>
      {children}
    </I18nContext.Provider>
  );
}

export function useI18n(): { dict: Dict; locale: Locale; currency: string } {
  const ctx = useContext(I18nContext);
  if (!ctx) throw new Error("useI18n must be used within LocaleProvider");
  return ctx;
}

/** 错误消息：优先按 code 查字典（与后端 Accept-Language 协商互为兜底） */
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
